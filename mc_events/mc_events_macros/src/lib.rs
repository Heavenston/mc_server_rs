use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, Ident, ItemFn, Type};

#[proc_macro_attribute]
pub fn event_callback(attr: TokenStream, item: TokenStream) -> TokenStream {
    let struct_name = parse_macro_input!(attr as Ident);
    let input = parse_macro_input!(item as ItemFn);

    let (event_type, event_name) = if let FnArg::Typed(pat_type) = &input.sig.inputs[0] {
        if let Type::Reference(ref_type) = &*pat_type.ty {
            (&*ref_type.elem, &*pat_type.pat)
        }
        else {
            panic!()
        }
    }
    else {
        panic!();
    };

    let visibility = input.vis;
    let block = &*input.block;

    let expanded = quote! {
        #visibility struct #struct_name;
        impl EventHandler for #struct_name {
            fn event_type(&self) -> TypeId {
                TypeId::of::<#event_type>()
            }
            fn on_event(&mut self, #event_name: &mut dyn Event) {
                let #event_name: &mut #event_type = event.downcast_mut::<#event_type>().unwrap();
                #block
            }
        }
    };

    TokenStream::from(expanded)
}
