use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Error, ItemFn, LitStr, Path, ReturnType, parse_macro_input};

#[proc_macro_derive(KernelTest, attributes(kernel_test))]
pub fn derive_kernel_test(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;

    let mut name: Option<LitStr> = None;
    let mut function: Option<Path> = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("kernel_test") {
            continue;
        }

        if let Err(err) = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let lit: LitStr = meta.value()?.parse()?;
                name = Some(lit);
                return Ok(());
            }
            if meta.path.is_ident("function") {
                let lit: LitStr = meta.value()?.parse()?;
                let parsed = lit
                    .parse::<Path>()
                    .map_err(|e| meta.error(format!("invalid function path: {e}")))?;
                function = Some(parsed);
                return Ok(());
            }
            Err(meta.error("expected `name` or `function`"))
        }) {
            return err.to_compile_error().into();
        }
    }

    let Some(function) = function else {
        return Error::new_spanned(
            ident,
            "missing #[kernel_test(function = \"path::to::fn\")] attribute",
        )
        .to_compile_error()
        .into();
    };

    let name = name.unwrap_or_else(|| LitStr::new(&ident.to_string(), ident.span()));
    let registration = format_ident!("__KERNEL_TEST_REGISTRATION_{}", ident);
    let shim = format_ident!("__kernel_test_shim_{}", ident);

    quote! {
        #[allow(non_snake_case)]
        extern "C" fn #shim() {
            #function();
        }

        #[allow(non_upper_case_globals)]
        #[used]
        #[cfg_attr(target_os = "none", unsafe(link_section = "kernel_tests"))]
        static #registration: ::kernel_tests::TestRegistration = ::kernel_tests::TestRegistration {
            name: ::kernel_tests::TestName::new(#name),
            run: #shim,
        };
    }
    .into()
}

#[proc_macro_attribute]
pub fn kernel_test(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut name: Option<LitStr> = None;

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("name") {
            let lit: LitStr = meta.value()?.parse()?;
            name = Some(lit);
            return Ok(());
        }
        Err(meta.error("expected `name`"))
    });

    parse_macro_input!(args with parser);
    let input_fn = parse_macro_input!(input as ItemFn);

    if !input_fn.sig.inputs.is_empty() {
        return Error::new_spanned(
            &input_fn.sig.inputs,
            "kernel test function must not accept arguments",
        )
        .to_compile_error()
        .into();
    }

    if !input_fn.sig.generics.params.is_empty() {
        return Error::new_spanned(
            &input_fn.sig.generics.params,
            "kernel test function must not have generics",
        )
        .to_compile_error()
        .into();
    }

    if input_fn.sig.asyncness.is_some() {
        return Error::new_spanned(
            &input_fn.sig.ident,
            "kernel test function must not be async",
        )
        .to_compile_error()
        .into();
    }

    if !matches!(input_fn.sig.output, ReturnType::Default) {
        return Error::new_spanned(
            &input_fn.sig.output,
            "kernel test function must return ()",
        )
        .to_compile_error()
        .into();
    }

    let ident = &input_fn.sig.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&ident.to_string(), ident.span()));
    let registration = format_ident!("__KERNEL_TEST_REGISTRATION_{}", ident);
    let shim = format_ident!("__kernel_test_shim_{}", ident);

    quote! {
        #input_fn

        #[allow(non_snake_case)]
        extern "C" fn #shim() {
            #ident();
        }

        #[allow(non_upper_case_globals)]
        #[used]
        #[cfg_attr(target_os = "none", unsafe(link_section = "kernel_tests"))]
        static #registration: ::kernel_tests::TestRegistration = ::kernel_tests::TestRegistration {
            name: ::kernel_tests::TestName::new(#name),
            run: #shim,
        };
    }
    .into()
}
