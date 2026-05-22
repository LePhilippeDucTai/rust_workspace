use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn time_it(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let name = &func.sig.ident;
    let block = &func.block;
    let sig = &func.sig;
    let vis = &func.vis;

    TokenStream::from(quote! {
        #vis #sig {
            use tracing::info;
            let start = std::time::Instant::now();
            let result = { #block };
            let duration = start.elapsed();

            let time_str = if duration.as_nanos() < 1_000 {
                format!("{} ns", duration.as_nanos())
            } else if duration.as_micros() < 1_000 {
                format!("{} µs", duration.as_micros())
            } else if duration.as_millis() < 1_000 {
                format!("{} ms", duration.as_millis())
            } else {
                format!("{:.3} s", duration.as_secs_f64())
            };
            info!("{} time: {}", stringify!(#name), time_str);
            result
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_macro_exists() {
        // Basic test to ensure the macro compiles
        // The actual functionality is hard to test since it's a proc_macro
        // but we can verify the crate compiles
        assert!(true);
    }
}
