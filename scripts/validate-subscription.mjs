const endpoint = process.argv[2] ?? process.env.MCP_ENDPOINT;
if (!endpoint) {
  throw new Error('usage: node validate-subscription.mjs <MCP endpoint>');
}

const timeoutMs = Number(process.env.MCP_SUBSCRIPTION_TIMEOUT_MS ?? '10000');
if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
  throw new Error('MCP_SUBSCRIPTION_TIMEOUT_MS must be a positive number');
}

// `subscriptions/listen` only exists in the final protocol, and that protocol
// requires an Mcp-Method header: without it `initialize` fails with -32020.
const FINAL_PROTOCOL = '2026-07-28';

const requestHeaders = (method) => ({
  'content-type': 'application/json',
  accept: 'application/json, text/event-stream',
  'mcp-method': method,
});

async function post(method, body, init = {}) {
  const res = await fetch(endpoint, {
    method: 'POST',
    headers: requestHeaders(method),
    body: JSON.stringify(body),
    ...init,
  });
  return res;
}

/**
 * The server must negotiate the final protocol, since the subscription stream
 * does not exist in the stable one. Asserted directly rather than inferred, so
 * a server that quietly falls back is a failure instead of a pass.
 */
async function assertFinalProtocol() {
  const res = await post('initialize', {
    jsonrpc: '2.0',
    id: 1,
    method: 'initialize',
    params: {
      protocolVersion: FINAL_PROTOCOL,
      capabilities: {},
      clientInfo: { name: 'cratesio-mcp-release-validation', version: '1.0.0' },
    },
  });
  const text = await res.text();
  if (!res.ok) {
    throw new Error(
      `initialize returned HTTP ${res.status}: ${text.slice(0, 300)}`,
    );
  }
  let negotiated;
  try {
    negotiated = JSON.parse(text)?.result?.protocolVersion;
  } catch {
    throw new Error(`initialize returned non-JSON: ${text.slice(0, 300)}`);
  }
  if (negotiated !== FINAL_PROTOCOL) {
    throw new Error(
      `server negotiated ${negotiated ?? 'nothing'}, expected ${FINAL_PROTOCOL}`,
    );
  }
}

/**
 * Open the subscription stream and prove it opened.
 *
 * This is the assertion the earlier version of this script was missing. A
 * server that does not implement the method answers this POST with a JSON-RPC
 * error over `application/json` rather than opening a stream, and that reply is
 * *faster* than the working case. A check that only races a concurrent
 * `tools/list` against a timeout therefore cannot see it: with no stream held
 * open there is nothing to be slow, so the broken server passes. That is how
 * #157 stayed green through a release.
 *
 * Returns the AbortController holding the stream; the caller aborts it.
 */
async function openSubscription() {
  const controller = new AbortController();
  const res = await post(
    'subscriptions/listen',
    { jsonrpc: '2.0', id: 2, method: 'subscriptions/listen', params: {} },
    { signal: controller.signal },
  );

  const contentType = res.headers.get('content-type') ?? '(none)';

  if (!res.ok) {
    controller.abort();
    throw new Error(
      `subscriptions/listen returned HTTP ${res.status}, expected 200 with an SSE stream`,
    );
  }

  if (!contentType.includes('text/event-stream')) {
    const body = await res.text().catch(() => '');
    controller.abort();
    throw new Error(
      `subscriptions/listen did not open a stream. ` +
        `content-type: ${contentType}; body: ${body.slice(0, 300)}`,
    );
  }

  return controller;
}

const moduleSpecifier =
  process.env.MCP_CLIENT_MODULE ?? '@modelcontextprotocol/client';

/**
 * The original concurrency check, now run while a stream is known to be open.
 *
 * The client exposes no way to ask whether a subscription is live, so this
 * cannot stand on its own; `openSubscription` supplies that half.
 */
async function validateConcurrentListTools() {
  const { Client, StreamableHTTPClientTransport } = await import(
    moduleSpecifier
  );

  const client = new Client(
    { name: 'cratesio-mcp-release-validation', version: '1.0.0' },
    {
      versionNegotiation: { mode: 'auto' },
      listChanged: {
        tools: { onChanged() {} },
      },
    },
  );

  // Collected rather than discarded. Closing an active subscription aborts its
  // fetch, which is expected cleanup, so those are filtered at the end instead
  // of every error being thrown away.
  const observed = [];
  client.onerror = (error) => observed.push(error);

  let timeout;
  try {
    await client.connect(new StreamableHTTPClientTransport(new URL(endpoint)));

    if (client.getProtocolEra() !== 'modern') {
      throw new Error('server did not negotiate the final MCP protocol');
    }

    const tools = await Promise.race([
      client.listTools(),
      new Promise((_, reject) => {
        timeout = setTimeout(
          () => reject(new Error(`tools/list timed out after ${timeoutMs}ms`)),
          timeoutMs,
        );
      }),
    ]);

    if (!tools.tools.some((tool) => tool.name === 'search_crates')) {
      throw new Error('tools/list did not include search_crates');
    }

    return { toolCount: tools.tools.length, observed };
  } finally {
    clearTimeout(timeout);
    await client.close();
  }
}

const isCleanupAbort = (error) => {
  const name = error?.name ?? '';
  const message = String(error?.message ?? error ?? '');
  return name === 'AbortError' || /abort/i.test(message);
};

await assertFinalProtocol();

const subscription = await openSubscription();
let result;
try {
  result = await validateConcurrentListTools();
} finally {
  subscription.abort();
}

const unexpected = result.observed.filter((error) => !isCleanupAbort(error));
for (const error of unexpected) {
  console.warn(
    `client reported: ${error?.message ?? JSON.stringify(error)}`,
  );
}

console.log(
  `Subscription concurrency validation passed ` +
    `(${result.toolCount} tools, stream confirmed open)`,
);
