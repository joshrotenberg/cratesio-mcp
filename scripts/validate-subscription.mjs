const endpoint = process.argv[2] ?? process.env.MCP_ENDPOINT;
if (!endpoint) {
  throw new Error('usage: node validate-subscription.mjs <MCP endpoint>');
}

const timeoutMs = Number(process.env.MCP_SUBSCRIPTION_TIMEOUT_MS ?? '10000');
if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
  throw new Error('MCP_SUBSCRIPTION_TIMEOUT_MS must be a positive number');
}

const moduleSpecifier =
  process.env.MCP_CLIENT_MODULE ?? '@modelcontextprotocol/client';
const { Client, StreamableHTTPClientTransport } = await import(moduleSpecifier);

const client = new Client(
  { name: 'cratesio-mcp-release-validation', version: '1.0.0' },
  {
    versionNegotiation: { mode: 'auto' },
    listChanged: {
      tools: { onChanged() {} },
    },
  },
);

// Closing an active subscription aborts its fetch. That is expected cleanup,
// not a validation failure, so only surfaced request failures are thrown.
client.onerror = () => {};

let timeout;
try {
  await client.connect(
    new StreamableHTTPClientTransport(new URL(endpoint)),
  );

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

  console.log(
    `Subscription concurrency validation passed (${tools.tools.length} tools)`,
  );
} finally {
  clearTimeout(timeout);
  await client.close();
}
