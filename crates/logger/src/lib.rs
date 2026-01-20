use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn logger(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;
    let name = sig.ident.to_string();

    let output = if sig.asyncness.is_some() {
        quote! {
            #vis #sig {
                let start = std::time::Instant::now();
                let result = async move #block.await;
                let elapsed = start.elapsed();
                log::info!(target: "commands", "{} executed in {:?}", #name, elapsed);
                result
            }
        }
    } else {
        quote! {
            #vis #sig {
                let start = std::time::Instant::now();
                let result = (|| #block)();
                let elapsed = start.elapsed();
                log::info!(target: "commands", "{} executed in {:?}", #name, elapsed);
                result
            }
        }
    };

    TokenStream::from(output)
}
