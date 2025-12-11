title:

过程宏中属性解析错误处理不完整

Category:

Macro Related

Description:

当用户传入错误的属性参数时，代码只是简单地返回syn::Result，但没有提供具体的错误上下文信息。比如当router参数不是有效的标识符时，用户很难知道具体哪里出错了。开发者在实际使用中会遇到调试困难的问题。

Background:

在开发Rust过程宏时，开发者经常需要解析用户传入的属性参数。这个代码片段实现了一个工具路由宏，用于自动生成路由函数。在实际项目中，用户可能会传入各种格式的参数，如果解析失败需要给出清晰的错误信息。

Problem Code/Description:

```rust
#[tool_router(router = "invalid-router-name")]
impl Handler {
    // ...
}

#[tool_router()]
impl Handler {
    // ...
}
```

Error Message:

```
error: proc-macro derive panicked
 --> tests/bad_macros.rs:10:1
  |
10 | #[tool_router()]
  | ^^^^^^^^^^^^^^^^
  |
  = help: message: Failed to parse tool_router attributes
```

```
error: expected identifier, found `-invalid-router-name`
 --> src/lib.rs:21:13
  |
21 |         fn #router_name() -> &'static str {
  |            ^^^^^^^^^^^^
```

Solution:

使用结构化错误处理提供清晰的错误信息。 ```rust use proc_macro::TokenStream; use syn::{ItemImpl, Error}; use quote::quote; fn parse_router_name(attr: TokenStream) -> Result<String, Error> { let attr_str = attr.to_string(); if attr_str.is_empty() { return Err(Error::new_spanned( attr, "Missing router name. Usage: #[tool_router(router = \"my_router\")]" )); } // Parse router = "name" format if !attr_str.contains("router") { return Err(Error::new_spanned( attr, "Missing 'router' parameter. Usage: #[tool_router(router = \"my_router\")]" )); } let parts: Vec<&str> = attr_str.split('=').collect(); if parts.len() < 2 { return Err(Error::new_spanned( attr, "Invalid format. Expected: router = \"name\"" )); } let router_name = parts[1].trim_matches('"').trim(); if router_name.is_empty() { return Err(Error::new_spanned( attr, "Router name cannot be empty" )); } if !router_name.chars().all(|c| c.is_alphanumeric() || c == '_') { return Err(Error::new_spanned( attr, format!("Router name '{}' contains invalid characters. Use only letters, numbers, and underscores", router_name) )); } Ok(router_name.to_string()) } #[proc_macro_attribute] pub fn tool_router(attr: TokenStream, input: TokenStream) -> TokenStream { let input_impl = match syn::parse::<ItemImpl>(input) { Ok(impl_block) => impl_block, Err(e) => return e.to_compile_error().into(), }; let router_name = match parse_router_name(attr) { Ok(name) => name, Err(e) => return e.to_compile_error().into(), }; let router_ident = syn::parse_str(&router_name).unwrap_or_else(|_| { syn::Ident::new("invalid_router", proc_macro2::Span::call_site()) }); let expanded = quote! { fn #router_ident() -> &'static str { "router" } #input_impl }; TokenStream::from(expanded) } ```

Attachments (1):

q51.zip
application/x-zip-compressed
