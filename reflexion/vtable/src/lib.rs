use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::{
    FnArg, GenericParam, Ident, ItemTrait, LitStr, PatType, ReturnType, Signature, TraitItem,
    parse_macro_input,
};

struct MemberFunctionSignature {
    name: Ident,
    abi: LitStr,
    generics: syn::Generics,
    mutability: bool,
    unsafety: bool,
    lifetime: Option<syn::Lifetime>,
    inputs: Vec<PatType>,
    output: ReturnType,
}

impl MemberFunctionSignature {
    fn new(signature: Signature) -> Result<Self> {
        let name = signature.ident;

        if let Some(asyncness) = signature.asyncness {
            return Err(build_eror(
                asyncness.span,
                "async function remain unsupported",
            ));
        }

        if signature.generics.params.iter().any(|param| match param {
            GenericParam::Lifetime(_) => false,
            _ => true,
        }) {
            return Err(build_eror(
                name.span(),
                "type and const generic are forbidden",
            ));
        }

        let abi = match signature.abi {
            None => Err(build_eror(name.span(), "ABI must be explicitly specified")),
            Some(syn::Abi {
                name: None,
                extern_token,
            }) => Err(build_eror(
                extern_token.span,
                "ABI must be explicitly specified",
            )),
            Some(syn::Abi {
                name: Some(abi_name),
                ..
            }) if abi_name.value() == "rust" => Err(build_eror(
                abi_name.span(),
                "rust ABI is unstable and therefor can't be used",
            )),
            Some(syn::Abi {
                name: Some(abi_name),
                ..
            }) => Ok(abi_name),
        }?;

        let (mutability, lifetime) = match signature.inputs.first() {
            None => Err(build_eror(
                name.span(),
                "function without any parameters aren't allowed",
            )),
            Some(FnArg::Typed(_)) => Err(build_eror(
                name.span(),
                "only methods are authorized, add `&self` or `&mut self` to the signature",
            )),
            Some(FnArg::Receiver(syn::Receiver {
                colon_token: Some(token),
                ..
            })) => Err(build_eror(
                token.span,
                "only `&self` or `&mut self` are authorized",
            )),
            Some(FnArg::Receiver(syn::Receiver {
                reference: None,
                self_token,
                ..
            })) => Err(build_eror(
                self_token.span,
                "only `&self` or `&mut self` are authorized",
            )),
            Some(FnArg::Receiver(syn::Receiver {
                reference: Some((_, lifetime)),
                mutability,
                ..
            })) => Ok((mutability.is_some(), lifetime.clone())),
        }?;

        let unsafety = signature.unsafety.is_some();

        let inputs: Vec<_> = signature
            .inputs
            .into_iter()
            .skip(1)
            .flat_map(|f| match f {
                FnArg::Receiver(_) => None,
                FnArg::Typed(arg) => Some(arg),
            })
            .collect();

        let output = signature.output; //TODO: safer return type

        Ok(Self {
            name,
            abi,
            generics: signature.generics,
            mutability,
            unsafety,
            lifetime,
            inputs,
            output,
        })
    }

    fn write_vtable_field(&self, opaque_ident: &Ident) -> TokenStream {
        let name = &self.name;
        let abi = &self.abi;
        let self_lifetime = &self.lifetime;
        let param: Vec<_> = self.inputs.iter().map(|pat| &*pat.ty).collect();

        let output = self.output.clone().into_token_stream();

        let lifetimes: Vec<_> = self.generics.lifetimes().map(|l| &l.lifetime).collect();

        let mut_token = self.mutability.then_some(quote! {mut});
        let unsafe_token = self.unsafety.then_some(quote! {unsafe});
        let impl_generics = (!lifetimes.is_empty()).then_some(quote! {for<#(#lifetimes,)*>});

        quote! {
            #name: #impl_generics #unsafe_token extern #abi fn(& #self_lifetime #mut_token #opaque_ident, #(#param,)* ) #output,
        }
    }

    fn write_handle_method(&self) -> TokenStream {
        let name = &self.name;
        let self_lifetime = &self.lifetime;
        let param = &self.inputs;
        let pats: Vec<_> = self.inputs.iter().map(|pat| &*pat.pat).collect();
        let output = self.output.clone().into_token_stream();
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();

        let mut_token = self.mutability.then_some(quote! {mut});
        let unsafe_token = self.unsafety.then_some(quote! {unsafe});
        let impl_generics = (!self.generics.params.is_empty()).then_some(quote! {#impl_generics});

        quote! {
            pub #unsafe_token fn #name #impl_generics(& #self_lifetime #mut_token self, #(#param,)* ) #output #where_clause {
                unsafe {
                    (self.vtable. #name)(self.handle, #(#pats,)*)
                }
            }
        }
    }
}

struct ExpensionBuilder {
    name: Ident,
    vtable_name: Ident,
    opaque_type: Ident,
    methods: Vec<MemberFunctionSignature>,
}

type Result<T> = std::result::Result<T, TokenStream>;

fn build_eror(span: Span, txt: impl AsRef<str>) -> TokenStream {
    let txt = txt.as_ref();
    quote_spanned! { span => compile_error!(#txt) }.into()
}

impl ExpensionBuilder {
    fn new(item_trait: ItemTrait) -> Result<Self> {
        let name = item_trait.ident;
        let methods: Result<Vec<_>> = item_trait
            .items
            .into_iter()
            .filter_map(|f| match f {
                TraitItem::Fn(item_fn) => Some(MemberFunctionSignature::new(item_fn.sig)),
                _ => None,
            })
            .collect();

        let vtable_name = format_ident!("{}Vtable", name);
        let ptr_type = format_ident!("{}Opaque", name);

        Ok(Self {
            name,
            vtable_name,
            opaque_type: ptr_type,
            methods: methods?,
        })
    }

    fn vtable_fields(&self) -> TokenStream {
        let implems: Vec<_> = self
            .methods
            .iter()
            .map(|method| method.write_vtable_field(&self.opaque_type))
            .collect();

        quote! {
            #(#implems)*
        }
    }

    fn mut_handle_methods(&self) -> TokenStream {
        let implems: Vec<_> = self
            .methods
            .iter()
            .map(|method| method.write_handle_method())
            .collect();

        quote! {
            #(#implems)*
        }
    }

    fn handle_methods(&self) -> TokenStream {
        let implems: Vec<_> = self
            .methods
            .iter()
            .flat_map(|method| (!method.mutability).then(|| method.write_handle_method()))
            .collect();

        quote! {
            #(#implems)*
        }
    }
}

fn vtable_impl(input: ItemTrait) -> Result<TokenStream> {
    let builder = ExpensionBuilder::new(input.clone())?;

    let name = &builder.name;
    let opaque_type = &builder.opaque_type;
    let trait_ext_name = format_ident!("{}VtableExt", name);
    let vtable_name = &builder.vtable_name;
    let vtable_fields = builder.vtable_fields();

    let method_names: Vec<_> = builder.methods.iter().map(|method| &method.name).collect();

    let mut_handle = format_ident!("{}MutHandle", name);
    let mut_handle_methods = builder.mut_handle_methods();
    let handle = format_ident!("{}Handle", name);
    let handle_methods = builder.handle_methods();

    let expanded = quote! {
        #input

        #[repr(C)]
        pub struct #opaque_type {
            _data: (),
            _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
        }

        #[repr(C)]
        struct #vtable_name {
            #vtable_fields
        }

        pub trait #trait_ext_name : #name {
            const VTABLE: #vtable_name = unsafe { #vtable_name {
                #(#method_names: ::std::mem::transmute(Self::#method_names as *const ()),)*
            }};

            fn as_handle(&self) -> #handle<'_> {
                unsafe {
                    let handle = self as *const Self as *const #opaque_type;
                    #handle {
                        handle: &*handle,
                        vtable: &Self::VTABLE,
                    }
                }
            }

            fn as_mut_handle<'a>(&'a mut self) -> #mut_handle<'a> {
                unsafe {
                    let handle = self as *mut Self as *mut #opaque_type;
                    #mut_handle {
                        handle: &mut *handle,
                        vtable: &Self::VTABLE,
                    }
                }
            }
        }

        impl<T: #name> #trait_ext_name for T {}

        #[repr(C)]
        pub struct #mut_handle<'handle_lifetime> {
            handle: &'handle_lifetime mut #opaque_type,
            vtable: &'static #vtable_name,
        }

        impl #mut_handle<'_> {
            #mut_handle_methods
        }

        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct #handle<'handle_lifetime> {
            handle: &'handle_lifetime #opaque_type,
            vtable: &'static #vtable_name,
        }

        impl #handle<'_> {
            #handle_methods
        }

    };
    Ok(expanded)
}

/// a utility macro to build explicit vtable for any trait,
/// the given trait should only containt "methods" shloudn't use async functions
// TODO: add support for functions where self appear multiple times ! which remaing unsupported for now
#[proc_macro_attribute]
pub fn vtable(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as ItemTrait);
    match vtable_impl(input) {
        Ok(stream) => stream,
        Err(stream) => stream,
    }
    .into()
}
