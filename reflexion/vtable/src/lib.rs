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
            return Err(build_error(
                asyncness.span,
                "async function remain unsupported",
            ));
        }

        if signature.generics.params.iter().any(|param| match param {
            GenericParam::Lifetime(_) => false,
            _ => true,
        }) {
            return Err(build_error(
                name.span(),
                "type and const generic are forbidden",
            ));
        }

        let abi = match signature.abi {
            None => Err(build_error(name.span(), "ABI must be explicitly specified")),
            Some(syn::Abi {
                name: None,
                extern_token,
            }) => Err(build_error(
                extern_token.span,
                "ABI must be explicitly specified",
            )),
            Some(syn::Abi {
                name: Some(abi_name),
                ..
            }) if abi_name.value() == "rust" => Err(build_error(
                abi_name.span(),
                "rust ABI is unstable and therefor can't be used",
            )),
            Some(syn::Abi {
                name: Some(abi_name),
                ..
            }) => Ok(abi_name),
        }?;

        let (mutability, lifetime) = match signature.inputs.first() {
            None => Err(build_error(
                name.span(),
                "function without any parameters aren't allowed",
            )),
            Some(FnArg::Typed(_)) => Err(build_error(
                name.span(),
                "only methods are authorized, add `&self` or `&mut self` to the signature",
            )),
            Some(FnArg::Receiver(syn::Receiver {
                colon_token: Some(token),
                ..
            })) => Err(build_error(
                token.span,
                "only `&self` or `&mut self` are authorized",
            )),
            Some(FnArg::Receiver(syn::Receiver {
                reference: None,
                self_token,
                ..
            })) => Err(build_error(
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
            pub #name: #impl_generics #unsafe_token extern #abi fn(& #self_lifetime #mut_token #opaque_ident, #(#param,)* ) #output,
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

struct ExpansionBuilder {
    name: Ident,
    vtable_name: Ident,
    opaque_type: Ident,
    methods: Vec<MemberFunctionSignature>,
}

type Result<T> = std::result::Result<T, TokenStream>;

fn build_error(span: Span, txt: impl AsRef<str>) -> TokenStream {
    let txt = txt.as_ref();
    quote_spanned! { span => compile_error!(#txt) }.into()
}

impl ExpansionBuilder {
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
    let builder = ExpansionBuilder::new(input.clone())?;

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

        #[doc = "An abstract type to create pointers and references to objects that implements ``#name``"]
        #[repr(C)]
        pub struct #opaque_type {
            _data: (),
            _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
        }

        impl ::reflexion::opaque::Opaque for #opaque_type {
            type Handle<'a> = #handle<'a>;
            type MutHandle<'a> = #mut_handle<'a>;
            type Vtable = #vtable_name;

            unsafe fn handle<'a>(handle: *const Self, vtable:  &'static Self::Vtable) -> Self::Handle<'a> {
                unsafe {
                    #handle {
                        handle: &*handle,
                        vtable,
                    }
                }
            }

            unsafe fn mut_handle<'a>(handle: *mut Self, vtable:  &'static Self::Vtable) -> Self::MutHandle<'a> {
                unsafe {
                    #mut_handle {
                        handle: &mut *handle,
                        vtable,
                    }
                }
            }
        }

        #[repr(C)]
        pub struct #vtable_name {
            #vtable_fields
        }

        pub trait #trait_ext_name : #name + Sized {
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

            fn boxed(self) -> ::reflexion::ffi_box::FfiBox<#opaque_type> {
                unsafe {
                    <::reflexion::ffi_box::FfiBox<#opaque_type>>::new(self, &Self::VTABLE)
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
            pub fn as_const(&self) -> #handle  {
                #handle {
                    handle: &self.handle,
                    vtable: &self.vtable,
                }
            }
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

        impl<'handle_lifetime> #handle<'handle_lifetime> {
            #handle_methods

            pub unsafe fn downcast<T: #name>(self) -> &'handle_lifetime T {
                unsafe {
                    let ptr = self.handle as *const #opaque_type as *const T;
                    &*ptr
                }
            }
        }

    };
    Ok(expanded)
}

/// a utility macro to build explicit vtable for any trait,
/// the given trait should only contain "methods" shloudn't use async functions
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
