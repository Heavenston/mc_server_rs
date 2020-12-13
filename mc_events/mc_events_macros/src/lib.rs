use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, Ident, ItemFn, Type, Pat};

#[proc_macro_attribute]
pub fn event_callback(attr: TokenStream, item: TokenStream) -> TokenStream {
    let struct_name = parse_macro_input!(attr as Ident);
    let input = parse_macro_input!(item as ItemFn);

    let mut inputs = input.sig.inputs.iter();

    let (event_type, event_name) = if let FnArg::Typed(pat_type) = inputs.next().unwrap() {
        if let Type::Reference(ref_type) = &*pat_type.ty {
            (&*ref_type.elem, &*pat_type.pat)
        }
        else {
            panic!("invalid event type")
        }
    }
    else {
        panic!("First argument must be an event");
    };

    let mut states = vec![];
    for arg in inputs {
        match arg {
            FnArg::Typed(pat_type) => {
                let ident = if let Pat::Ident(ident) = &*pat_type.pat { ident } else {panic!("Invalid argument")};
                let (is_mut, type_) = if let Type::Reference(reference) = &*pat_type.ty { (&reference.mutability, &*reference.elem) } else { panic!("Invalid argument") };
                states.push((is_mut, &ident.ident, type_));
            }
            _ => panic!("Invalid argument"),
        }
    }

    let visibility = input.vis;
    let block = &*input.block;

    let expanded = if states.is_empty() {
        quote! {
            #visibility struct #struct_name;
            impl EventHandler for #struct_name {
                fn event_type(&self) -> TypeId {
                    TypeId::of::<#event_type>()
                }
                fn on_event(&mut self, #event_name: &mut dyn Event) {
                    let #event_name: &mut #event_type = #event_name.downcast_mut::<#event_type>().unwrap();
                    #block
                }
            }
        }
    }
    else {
        let states_fields = states.iter().map(|(_, n, v)| quote! {
            pub #n: #v
        });
        let states_values = states.iter().map(|(is_mut, n, v)| quote! {
            let #n: &#is_mut #v = &#is_mut self.#n;
        });
        quote! {
            #visibility struct #struct_name {
                #(#states_fields),*
            }
            impl EventHandler for #struct_name {
                fn event_type(&self) -> TypeId {
                    TypeId::of::<#event_type>()
                }
                fn on_event(&mut self, #event_name: &mut dyn Event) {
                    let #event_name: &mut #event_type = #event_name.downcast_mut::<#event_type>().unwrap();
                    #(#states_values)*
                    #block
                }
            }
        }
    };

    TokenStream::from(expanded)
}
