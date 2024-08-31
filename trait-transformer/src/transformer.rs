// Copyright (c) 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Comma,
    AngleBracketedGenericArguments, Ident, ItemImpl, ItemTrait, Path, PathSegment, Result,
    ReturnType, Token, TraitBound, TraitBoundModifier, TraitItem, Type, TypeParam,
};

struct Attrs {
    traits: Punctuated<Transform, Comma>,
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            traits: input.parse_terminated(Transform::parse, Token![,])?,
        })
    }
}

struct Transform {
    subtrait_name: Ident,
    #[allow(dead_code)]
    colon: Token![:],
    subtrait: Path,
}

impl Parse for Transform {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            subtrait_name: input.parse()?,
            colon: input.parse()?,
            subtrait: input.parse()?,
        })
    }
}

pub fn trait_transformer(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attrs = parse_macro_input!(attr as Attrs);
    let item = parse_macro_input!(item as ItemTrait);

    let transformed_trait = transform_trait(&attrs, &item);
    let output = quote! {
        #item
        #transformed_trait
    };

    output.into()
}

fn transform_trait(attrs: &Attrs, tr: &ItemTrait) -> TokenStream {
    let traits = attrs
        .traits
        .iter()
        .map(|attr| {
            let subtrait = &attr.subtrait;
            let fn_bounds = tr.items.iter().filter_map(|item| {
                match item {
                    TraitItem::Fn(item_fn) => {
                        let is_async = item_fn.sig.asyncness.is_some();
                        let returns_impl_trait =
                            if let ReturnType::Type(_, ty) = &item_fn.sig.output {
                                matches!(**ty, Type::ImplTrait(_))
                            } else {
                                false
                            };

                        if is_async || returns_impl_trait {
                            let name = &item_fn.sig.ident;
                            return Some(quote! { #name(..): #subtrait });
                        }
                    }
                    _ => (),
                }
                None
            });

            let tr_ident = &tr.ident;
            let supertrait = syn::TypeParamBound::Verbatim(quote! {
                #tr_ident<#(#fn_bounds),*>
            });
            let bounds = Punctuated::from_iter(
                vec![
                    supertrait,
                    syn::TypeParamBound::Trait(TraitBound {
                        paren_token: None,
                        modifier: TraitBoundModifier::None,
                        lifetimes: None,
                        path: attr.subtrait.clone(),
                    }),
                ]
                .into_iter(),
            );
            let trai = ItemTrait {
                attrs: Vec::new(),
                ident: attr.subtrait_name.clone(),
                items: Vec::new(),
                supertraits: bounds.clone(),
                ..tr.clone()
            };
            // match attr.auto {
            //     None => quote! {
            //         #trai
            //     },
            //     Some(_) => {
            let mut g = tr.generics.clone();
            g.params.push(syn::GenericParam::Type(TypeParam {
                attrs: Vec::new(),
                ident: format_ident!("__WSelf"),
                colon_token: None,
                bounds: bounds,
                eq_token: None,
                default: None,
            }));
            let (a, b, c) = tr.generics.split_for_impl();
            let mut path: Path = Path {
                leading_colon: None,
                segments: Punctuated::from_iter(
                    vec![PathSegment {
                        ident: attr.subtrait_name.clone(),
                        arguments: syn::PathArguments::AngleBracketed(
                            syn::parse2(quote! {
                                <#b>
                            })
                            .unwrap(),
                        ),
                    }]
                    .into_iter(),
                ),
            };
            let imple = ItemImpl {
                attrs: Vec::new(),
                defaultness: None,
                unsafety: tr.unsafety.clone(),
                impl_token: Default::default(),
                generics: g,
                trait_: Some((None, path, Default::default())),
                self_ty: Box::new(Type::Path(syn::TypePath {
                    qself: None,
                    path: format_ident!("__WSelf").into(),
                })),
                brace_token: Default::default(),
                items: vec![],
            };
            quote! {
                #trai
                #imple
            }

            // }
        })
        .collect::<Vec<_>>();

    quote! { #(#traits)* }
}
