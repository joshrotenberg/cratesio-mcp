//! Format rustdoc items into LLM-readable markdown text.

use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, Crate, DynTrait, Enum, Function, FunctionPointer,
    GenericArg, GenericArgs, GenericBound, GenericParamDef, GenericParamDefKind, Generics, Id,
    Item, ItemEnum, Struct, StructKind, Term, Trait, Type, VariantKind, Visibility,
};

/// Format a module listing showing grouped children with summaries.
pub fn format_module_listing(krate: &Crate, module_id: &Id) -> String {
    let module_item = match krate.index.get(module_id) {
        Some(item) => item,
        None => return "Module not found in index.".to_string(),
    };

    let children = match &module_item.inner {
        ItemEnum::Module(m) => &m.items,
        _ => return "Item is not a module.".to_string(),
    };

    let module_name = module_item.name.as_deref().unwrap_or("(root)");

    let mut output = String::new();
    output.push_str(&format!("# Module `{}`\n\n", module_name));

    if let Some(docs) = &module_item.docs {
        let summary = first_sentence(docs);
        if !summary.is_empty() {
            output.push_str(&format!("{}\n\n", summary));
        }
    }

    // Group children by kind
    let mut modules = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut traits = Vec::new();
    let mut functions = Vec::new();
    let mut type_aliases = Vec::new();
    let mut constants = Vec::new();
    let mut macros = Vec::new();
    let mut other = Vec::new();

    for child_id in children {
        let Some(child) = krate.index.get(child_id) else {
            continue;
        };
        // Skip non-public items
        if !matches!(child.visibility, Visibility::Public) {
            continue;
        }
        match &child.inner {
            ItemEnum::Module(_) => modules.push(child),
            ItemEnum::Struct(_) => structs.push(child),
            ItemEnum::Enum(_) => enums.push(child),
            ItemEnum::Trait(_) => traits.push(child),
            ItemEnum::Function(_) => functions.push(child),
            ItemEnum::TypeAlias(_) => type_aliases.push(child),
            ItemEnum::Constant { .. } => constants.push(child),
            ItemEnum::Macro(_) | ItemEnum::ProcMacro(_) => macros.push(child),
            ItemEnum::Use(_) | ItemEnum::ExternCrate { .. } => {}
            _ => other.push(child),
        }
    }

    fn write_section(output: &mut String, heading: &str, items: &[&Item]) {
        if items.is_empty() {
            return;
        }
        output.push_str(&format!("## {}\n\n", heading));
        for item in items {
            let name = item.name.as_deref().unwrap_or("_");
            let summary = item.docs.as_deref().map(first_sentence).unwrap_or_default();
            if summary.is_empty() {
                output.push_str(&format!("- `{}`\n", name));
            } else {
                output.push_str(&format!("- `{}` -- {}\n", name, summary));
            }
        }
        output.push('\n');
    }

    write_section(&mut output, "Modules", &modules);
    write_section(&mut output, "Traits", &traits);
    write_section(&mut output, "Structs", &structs);
    write_section(&mut output, "Enums", &enums);
    write_section(&mut output, "Functions", &functions);
    write_section(&mut output, "Type Aliases", &type_aliases);
    write_section(&mut output, "Constants", &constants);
    write_section(&mut output, "Macros", &macros);
    write_section(&mut output, "Other", &other);

    output
}

/// Format full documentation for a specific item.
pub fn format_item_detail(krate: &Crate, item: &Item) -> String {
    let name = item.name.as_deref().unwrap_or("_");
    let mut output = String::new();

    match &item.inner {
        ItemEnum::Function(f) => {
            output.push_str(&format!("# Function `{}`\n\n", name));
            output.push_str("```rust\n");
            output.push_str(&format_function_signature(name, f));
            output.push_str("\n```\n\n");
        }
        ItemEnum::Struct(s) => {
            output.push_str(&format!("# Struct `{}`\n\n", name));
            output.push_str("```rust\n");
            output.push_str(&format_struct_definition(krate, name, s));
            output.push_str("\n```\n\n");
            format_struct_methods(krate, s, &mut output);
        }
        ItemEnum::Enum(e) => {
            output.push_str(&format!("# Enum `{}`\n\n", name));
            output.push_str("```rust\n");
            output.push_str(&format_enum_definition(krate, name, e));
            output.push_str("\n```\n\n");
        }
        ItemEnum::Trait(t) => {
            output.push_str(&format!("# Trait `{}`\n\n", name));
            output.push_str("```rust\n");
            output.push_str(&format_trait_definition(krate, name, t));
            output.push_str("\n```\n\n");
        }
        ItemEnum::TypeAlias(ta) => {
            output.push_str(&format!("# Type Alias `{}`\n\n", name));
            output.push_str("```rust\n");
            output.push_str(&format!(
                "type {}{} = {};\n",
                name,
                format_generics(&ta.generics),
                format_type(&ta.type_)
            ));
            output.push_str("```\n\n");
        }
        ItemEnum::Constant { type_, const_ } => {
            output.push_str(&format!("# Constant `{}`\n\n", name));
            output.push_str("```rust\n");
            output.push_str(&format!(
                "const {}: {} = {};\n",
                name,
                format_type(type_),
                const_.expr
            ));
            output.push_str("```\n\n");
        }
        ItemEnum::Macro(body) => {
            output.push_str(&format!("# Macro `{}`\n\n", name));
            output.push_str("```rust\n");
            output.push_str(body);
            output.push_str("\n```\n\n");
        }
        _ => {
            output.push_str(&format!("# `{}`\n\n", name));
        }
    }

    if let Some(docs) = &item.docs {
        // Cap at 200 lines
        let lines: Vec<&str> = docs.lines().collect();
        if lines.len() > 200 {
            for line in &lines[..200] {
                output.push_str(line);
                output.push('\n');
            }
            output.push_str("\n... (truncated)\n");
        } else {
            output.push_str(docs);
            output.push('\n');
        }
    }

    output
}

/// Format search results as a numbered list.
pub fn format_search_results(krate: &Crate, matches: &[(&Id, &Item)]) -> String {
    let mut output = String::new();

    for (i, (id, item)) in matches.iter().enumerate() {
        let name = item.name.as_deref().unwrap_or("_");
        let kind = item_kind_label(&item.inner);
        let path = item_path(krate, id);
        let summary = item.docs.as_deref().map(first_sentence).unwrap_or_default();

        output.push_str(&format!("{}. [{}] `{}`", i + 1, kind, path));
        if !summary.is_empty() {
            output.push_str(&format!(" -- {}", summary));
        }
        output.push('\n');

        // Show brief signature for functions
        if let ItemEnum::Function(f) = &item.inner {
            output.push_str(&format!(
                "   `{}`\n",
                format_function_signature(name, f).trim()
            ));
        }
    }

    output
}

/// Reconstruct the fully qualified path for an item.
pub fn item_path(krate: &Crate, id: &Id) -> String {
    if let Some(summary) = krate.paths.get(id) {
        summary.path.join("::")
    } else if let Some(item) = krate.index.get(id) {
        item.name.clone().unwrap_or_else(|| "_".to_string())
    } else {
        "_".to_string()
    }
}

/// Resolve an item path string (e.g. "de::from_str") against the crate index.
///
/// Walks from the root module, splitting on `::`.
pub fn resolve_item_path<'a>(krate: &'a Crate, path: &str) -> Option<&'a Item> {
    let segments: Vec<&str> = path.split("::").collect();

    // Strategy 1: Walk from root module through nested modules
    if let Some(item) = walk_modules(krate, &krate.root, &segments) {
        return Some(item);
    }

    // Strategy 2: Search by name match on the last segment, then verify path
    let target = segments.last()?;
    for item in krate.index.values() {
        if item.crate_id != 0 {
            continue;
        }
        if item.name.as_deref() == Some(target) {
            if segments.len() == 1 {
                return Some(item);
            }
            // Verify the path matches
            if let Some(summary) = krate.paths.get(&item.id) {
                let item_path: Vec<&str> = summary.path.iter().map(|s| s.as_str()).collect();
                if item_path.ends_with(&segments) {
                    return Some(item);
                }
            }
        }
    }

    None
}

/// Walk through nested modules to find an item.
fn walk_modules<'a>(krate: &'a Crate, module_id: &Id, segments: &[&str]) -> Option<&'a Item> {
    if segments.is_empty() {
        return krate.index.get(module_id);
    }

    let module_item = krate.index.get(module_id)?;
    let children = match &module_item.inner {
        ItemEnum::Module(m) => &m.items,
        _ => return None,
    };

    let target = segments[0];
    for child_id in children {
        let child = krate.index.get(child_id)?;
        if child.name.as_deref() == Some(target) {
            if segments.len() == 1 {
                return Some(child);
            }
            // Try to descend into this as a module
            if matches!(child.inner, ItemEnum::Module(_))
                && let Some(found) = walk_modules(krate, child_id, &segments[1..])
            {
                return Some(found);
            }
        }
    }

    None
}

/// Resolve a module path (e.g. "de" or "de::value") to a module Id.
pub fn resolve_module_path(krate: &Crate, path: &str) -> Option<Id> {
    let segments: Vec<&str> = path.split("::").collect();
    let mut current_id = krate.root;

    for segment in &segments {
        let module_item = krate.index.get(&current_id)?;
        let children = match &module_item.inner {
            ItemEnum::Module(m) => &m.items,
            _ => return None,
        };

        let mut found = false;
        for child_id in children {
            if let Some(child) = krate.index.get(child_id)
                && child.name.as_deref() == Some(segment)
                && matches!(child.inner, ItemEnum::Module(_))
            {
                current_id = *child_id;
                found = true;
                break;
            }
        }
        if !found {
            return None;
        }
    }

    Some(current_id)
}

// ── Type formatting ────────────────────────────────────────────────────

/// Render a `Type` to a human-readable string.
pub fn format_type(ty: &Type) -> String {
    match ty {
        Type::Primitive(name) => name.clone(),
        Type::Generic(name) => name.clone(),
        Type::ResolvedPath(path) => {
            let mut s = path.path.clone();
            if let Some(args) = &path.args {
                s.push_str(&format_generic_args(args));
            }
            s
        }
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            let mut s = String::from("&");
            if let Some(lt) = lifetime {
                s.push_str(lt);
                s.push(' ');
            }
            if *is_mutable {
                s.push_str("mut ");
            }
            s.push_str(&format_type(type_));
            s
        }
        Type::Tuple(types) => {
            if types.is_empty() {
                "()".to_string()
            } else {
                let inner: Vec<String> = types.iter().map(format_type).collect();
                format!("({})", inner.join(", "))
            }
        }
        Type::Slice(ty) => format!("[{}]", format_type(ty)),
        Type::Array { type_, len } => format!("[{}; {}]", format_type(type_), len),
        Type::RawPointer { is_mutable, type_ } => {
            if *is_mutable {
                format!("*mut {}", format_type(type_))
            } else {
                format!("*const {}", format_type(type_))
            }
        }
        Type::ImplTrait(bounds) => format!("impl {}", format_bounds(bounds)),
        Type::DynTrait(dyn_trait) => format_dyn_trait(dyn_trait),
        Type::FunctionPointer(fp) => format_fn_pointer(fp),
        Type::QualifiedPath {
            name,
            self_type,
            trait_,
            ..
        } => {
            if let Some(t) = trait_ {
                format!("<{} as {}>::{}", format_type(self_type), t.path, name)
            } else {
                format!("<{}>::{}", format_type(self_type), name)
            }
        }
        Type::Infer => "_".to_string(),
        Type::Pat { type_, .. } => format_type(type_),
    }
}

fn format_generic_args(args: &GenericArgs) -> String {
    match args {
        GenericArgs::AngleBracketed {
            args, constraints, ..
        } => {
            let mut parts: Vec<String> = args.iter().map(format_generic_arg).collect();
            for c in constraints {
                parts.push(format_assoc_constraint(c));
            }
            if parts.is_empty() {
                String::new()
            } else {
                format!("<{}>", parts.join(", "))
            }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let input_str: Vec<String> = inputs.iter().map(format_type).collect();
            let mut s = format!("({})", input_str.join(", "));
            if let Some(out) = output {
                s.push_str(&format!(" -> {}", format_type(out)));
            }
            s
        }
        GenericArgs::ReturnTypeNotation => "(..)".to_string(),
    }
}

fn format_generic_arg(arg: &GenericArg) -> String {
    match arg {
        GenericArg::Lifetime(lt) => lt.clone(),
        GenericArg::Type(ty) => format_type(ty),
        GenericArg::Const(c) => c.expr.clone(),
        GenericArg::Infer => "_".to_string(),
    }
}

fn format_assoc_constraint(c: &AssocItemConstraint) -> String {
    match &c.binding {
        AssocItemConstraintKind::Equality(term) => {
            let val = match term {
                Term::Type(ty) => format_type(ty),
                Term::Constant(c) => c.expr.clone(),
            };
            format!("{} = {}", c.name, val)
        }
        AssocItemConstraintKind::Constraint(bounds) => {
            format!("{}: {}", c.name, format_bounds(bounds))
        }
    }
}

fn format_bounds(bounds: &[GenericBound]) -> String {
    let parts: Vec<String> = bounds
        .iter()
        .map(|b| match b {
            GenericBound::TraitBound { trait_, .. } => {
                let mut s = trait_.path.clone();
                if let Some(args) = &trait_.args {
                    s.push_str(&format_generic_args(args));
                }
                s
            }
            GenericBound::Outlives(lt) => lt.clone(),
            GenericBound::Use(_) => "use<..>".to_string(),
        })
        .collect();
    parts.join(" + ")
}

fn format_dyn_trait(dt: &DynTrait) -> String {
    let mut parts: Vec<String> = dt
        .traits
        .iter()
        .map(|pt| {
            let mut s = pt.trait_.path.clone();
            if let Some(args) = &pt.trait_.args {
                s.push_str(&format_generic_args(args));
            }
            s
        })
        .collect();
    if let Some(lt) = &dt.lifetime {
        parts.push(lt.clone());
    }
    format!("dyn {}", parts.join(" + "))
}

fn format_fn_pointer(fp: &FunctionPointer) -> String {
    let inputs: Vec<String> = fp
        .sig
        .inputs
        .iter()
        .map(|(_, ty)| format_type(ty))
        .collect();
    let mut s = format!("fn({})", inputs.join(", "));
    if let Some(out) = &fp.sig.output {
        s.push_str(&format!(" -> {}", format_type(out)));
    }
    s
}

// ── Signature formatting ───────────────────────────────────────────────

fn format_function_signature(name: &str, f: &Function) -> String {
    let mut s = String::new();
    if f.header.is_const {
        s.push_str("const ");
    }
    if f.header.is_async {
        s.push_str("async ");
    }
    if f.header.is_unsafe {
        s.push_str("unsafe ");
    }
    s.push_str("fn ");
    s.push_str(name);
    s.push_str(&format_generics(&f.generics));
    s.push('(');
    let params: Vec<String> = f
        .sig
        .inputs
        .iter()
        .map(|(param_name, ty)| format!("{}: {}", param_name, format_type(ty)))
        .collect();
    s.push_str(&params.join(", "));
    s.push(')');
    if let Some(ret) = &f.sig.output {
        s.push_str(&format!(" -> {}", format_type(ret)));
    }
    s.push_str(&format_where_clause(&f.generics));
    s
}

fn format_generics(g: &Generics) -> String {
    if g.params.is_empty() {
        return String::new();
    }
    let params: Vec<String> = g
        .params
        .iter()
        .filter(|p| {
            !matches!(
                p.kind,
                GenericParamDefKind::Type {
                    is_synthetic: true,
                    ..
                }
            )
        })
        .map(format_generic_param)
        .collect();
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

fn format_generic_param(p: &GenericParamDef) -> String {
    match &p.kind {
        GenericParamDefKind::Lifetime { .. } => p.name.clone(),
        GenericParamDefKind::Type {
            bounds, default, ..
        } => {
            let mut s = p.name.clone();
            if !bounds.is_empty() {
                s.push_str(&format!(": {}", format_bounds(bounds)));
            }
            if let Some(def) = default {
                s.push_str(&format!(" = {}", format_type(def)));
            }
            s
        }
        GenericParamDefKind::Const { type_, default } => {
            let mut s = format!("const {}: {}", p.name, format_type(type_));
            if let Some(def) = default {
                s.push_str(&format!(" = {}", def));
            }
            s
        }
    }
}

fn format_where_clause(g: &Generics) -> String {
    if g.where_predicates.is_empty() {
        return String::new();
    }
    let preds: Vec<String> = g
        .where_predicates
        .iter()
        .map(|wp| match wp {
            rustdoc_types::WherePredicate::BoundPredicate { type_, bounds, .. } => {
                format!("{}: {}", format_type(type_), format_bounds(bounds))
            }
            rustdoc_types::WherePredicate::LifetimePredicate {
                lifetime, outlives, ..
            } => {
                format!("{}: {}", lifetime, outlives.join(" + "))
            }
            rustdoc_types::WherePredicate::EqPredicate { lhs, rhs } => {
                let rhs_str = match rhs {
                    Term::Type(ty) => format_type(ty),
                    Term::Constant(c) => c.expr.clone(),
                };
                format!("{} = {}", format_type(lhs), rhs_str)
            }
        })
        .collect();
    format!("\nwhere\n    {}", preds.join(",\n    "))
}

fn format_struct_definition(krate: &Crate, name: &str, s: &Struct) -> String {
    let mut out = format!("struct {}{}", name, format_generics(&s.generics));
    match &s.kind {
        StructKind::Unit => {
            out.push(';');
        }
        StructKind::Tuple(fields) => {
            out.push('(');
            let field_strs: Vec<String> = fields
                .iter()
                .map(|f| {
                    f.as_ref()
                        .and_then(|id| krate.index.get(id))
                        .map(|item| match &item.inner {
                            ItemEnum::StructField(ty) => format_type(ty),
                            _ => "_".to_string(),
                        })
                        .unwrap_or_else(|| "/* private */".to_string())
                })
                .collect();
            out.push_str(&field_strs.join(", "));
            out.push_str(");");
        }
        StructKind::Plain { fields, .. } => {
            out.push_str(&format_where_clause(&s.generics));
            out.push_str(" {\n");
            for field_id in fields {
                if let Some(field) = krate.index.get(field_id) {
                    let field_name = field.name.as_deref().unwrap_or("_");
                    if let ItemEnum::StructField(ty) = &field.inner {
                        out.push_str(&format!("    pub {}: {},\n", field_name, format_type(ty)));
                    }
                }
            }
            out.push('}');
        }
    }
    out
}

fn format_struct_methods(krate: &Crate, s: &Struct, output: &mut String) {
    // Find inherent impls
    let mut methods: Vec<(&str, String)> = Vec::new();
    for impl_id in &s.impls {
        let Some(impl_item) = krate.index.get(impl_id) else {
            continue;
        };
        let ItemEnum::Impl(imp) = &impl_item.inner else {
            continue;
        };
        // Only inherent impls (no trait)
        if imp.trait_.is_some() {
            continue;
        }
        for method_id in &imp.items {
            let Some(method) = krate.index.get(method_id) else {
                continue;
            };
            if !matches!(method.visibility, Visibility::Public) {
                continue;
            }
            let name = method.name.as_deref().unwrap_or("_");
            if let ItemEnum::Function(f) = &method.inner {
                let sig = format_function_signature(name, f);
                methods.push((name, sig));
            }
        }
    }

    if !methods.is_empty() {
        output.push_str("## Methods\n\n");
        for (name, sig) in &methods {
            output.push_str(&format!("- `{}`\n  ```rust\n  {}\n  ```\n", name, sig));
        }
        output.push('\n');
    }
}

fn format_enum_definition(krate: &Crate, name: &str, e: &Enum) -> String {
    let mut out = format!("enum {}{}", name, format_generics(&e.generics));
    out.push_str(&format_where_clause(&e.generics));
    out.push_str(" {\n");
    for variant_id in &e.variants {
        if let Some(variant_item) = krate.index.get(variant_id) {
            let vname = variant_item.name.as_deref().unwrap_or("_");
            if let ItemEnum::Variant(v) = &variant_item.inner {
                out.push_str(&format!("    {}", vname));
                match &v.kind {
                    VariantKind::Plain => {}
                    VariantKind::Tuple(fields) => {
                        out.push('(');
                        let field_strs: Vec<String> = fields
                            .iter()
                            .map(|f| {
                                f.as_ref()
                                    .and_then(|id| krate.index.get(id))
                                    .map(|item| match &item.inner {
                                        ItemEnum::StructField(ty) => format_type(ty),
                                        _ => "_".to_string(),
                                    })
                                    .unwrap_or_else(|| "/* private */".to_string())
                            })
                            .collect();
                        out.push_str(&field_strs.join(", "));
                        out.push(')');
                    }
                    VariantKind::Struct { fields, .. } => {
                        out.push_str(" {\n");
                        for field_id in fields {
                            if let Some(field) = krate.index.get(field_id) {
                                let fname = field.name.as_deref().unwrap_or("_");
                                if let ItemEnum::StructField(ty) = &field.inner {
                                    out.push_str(&format!(
                                        "        {}: {},\n",
                                        fname,
                                        format_type(ty)
                                    ));
                                }
                            }
                        }
                        out.push_str("    }");
                    }
                }
                out.push_str(",\n");
            }
        }
    }
    out.push('}');
    out
}

fn format_trait_definition(krate: &Crate, name: &str, t: &Trait) -> String {
    let mut out = String::new();
    if t.is_unsafe {
        out.push_str("unsafe ");
    }
    out.push_str(&format!("trait {}{}", name, format_generics(&t.generics)));
    if !t.bounds.is_empty() {
        out.push_str(&format!(": {}", format_bounds(&t.bounds)));
    }
    out.push_str(&format_where_clause(&t.generics));
    out.push_str(" {\n");
    for item_id in &t.items {
        if let Some(item) = krate.index.get(item_id) {
            let iname = item.name.as_deref().unwrap_or("_");
            match &item.inner {
                ItemEnum::Function(f) => {
                    let sig = format_function_signature(iname, f);
                    if f.has_body {
                        out.push_str(&format!("    {} {{ ... }}\n", sig));
                    } else {
                        out.push_str(&format!("    {};\n", sig));
                    }
                }
                ItemEnum::AssocType {
                    bounds,
                    type_: default,
                    ..
                } => {
                    out.push_str(&format!("    type {}", iname));
                    if !bounds.is_empty() {
                        out.push_str(&format!(": {}", format_bounds(bounds)));
                    }
                    if let Some(def) = default {
                        out.push_str(&format!(" = {}", format_type(def)));
                    }
                    out.push_str(";\n");
                }
                ItemEnum::AssocConst { type_, .. } => {
                    out.push_str(&format!("    const {}: {};\n", iname, format_type(type_)));
                }
                _ => {}
            }
        }
    }
    out.push('}');
    out
}

// ── Helpers ────────────────────────────────────────────────────────────

fn item_kind_label(inner: &ItemEnum) -> &'static str {
    match inner {
        ItemEnum::Module(_) => "mod",
        ItemEnum::Function(_) => "fn",
        ItemEnum::Struct(_) => "struct",
        ItemEnum::Enum(_) => "enum",
        ItemEnum::Trait(_) => "trait",
        ItemEnum::TypeAlias(_) => "type",
        ItemEnum::Constant { .. } => "const",
        ItemEnum::Macro(_) => "macro",
        ItemEnum::ProcMacro(_) => "proc_macro",
        ItemEnum::Union(_) => "union",
        ItemEnum::Static(_) => "static",
        ItemEnum::Variant(_) => "variant",
        ItemEnum::StructField(_) => "field",
        ItemEnum::Impl(_) => "impl",
        ItemEnum::Use(_) => "use",
        ItemEnum::ExternCrate { .. } => "extern_crate",
        ItemEnum::TraitAlias(_) => "trait_alias",
        ItemEnum::ExternType => "extern_type",
        ItemEnum::AssocConst { .. } => "assoc_const",
        ItemEnum::AssocType { .. } => "assoc_type",
        ItemEnum::Primitive(_) => "primitive",
    }
}

/// Extract the first sentence from a doc string.
fn first_sentence(docs: &str) -> String {
    let first_line = docs.lines().next().unwrap_or("");
    // Truncate at first period followed by space or end
    if let Some(pos) = first_line.find(". ") {
        first_line[..=pos].to_string()
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustdoc_types::*;

    #[test]
    fn first_sentence_extracts_correctly() {
        assert_eq!(first_sentence("Hello world. More text."), "Hello world.");
        assert_eq!(first_sentence("Single line"), "Single line");
        assert_eq!(first_sentence("Ends with period."), "Ends with period.");
        assert_eq!(first_sentence("Line one.\nLine two."), "Line one.");
    }

    #[test]
    fn format_primitive_type() {
        assert_eq!(format_type(&Type::Primitive("i32".to_string())), "i32");
    }

    #[test]
    fn format_generic_type() {
        assert_eq!(format_type(&Type::Generic("T".to_string())), "T");
    }

    #[test]
    fn format_borrowed_ref() {
        let ty = Type::BorrowedRef {
            lifetime: Some("'a".to_string()),
            is_mutable: true,
            type_: Box::new(Type::Primitive("str".to_string())),
        };
        assert_eq!(format_type(&ty), "&'a mut str");
    }

    #[test]
    fn format_tuple_type() {
        let ty = Type::Tuple(vec![
            Type::Primitive("i32".to_string()),
            Type::Primitive("bool".to_string()),
        ]);
        assert_eq!(format_type(&ty), "(i32, bool)");
    }

    #[test]
    fn format_unit_type() {
        assert_eq!(format_type(&Type::Tuple(vec![])), "()");
    }

    #[test]
    fn format_slice_type() {
        let ty = Type::Slice(Box::new(Type::Primitive("u8".to_string())));
        assert_eq!(format_type(&ty), "[u8]");
    }

    #[test]
    fn format_array_type() {
        let ty = Type::Array {
            type_: Box::new(Type::Primitive("u8".to_string())),
            len: "32".to_string(),
        };
        assert_eq!(format_type(&ty), "[u8; 32]");
    }

    #[test]
    fn format_raw_pointer() {
        let ty = Type::RawPointer {
            is_mutable: false,
            type_: Box::new(Type::Primitive("u8".to_string())),
        };
        assert_eq!(format_type(&ty), "*const u8");
    }

    #[test]
    fn item_kind_labels() {
        assert_eq!(
            item_kind_label(&ItemEnum::Module(Module {
                items: vec![],
                is_stripped: false,
                is_crate: false,
            })),
            "mod"
        );
    }
}
