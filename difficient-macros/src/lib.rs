extern crate proc_macro;

use darling::{
    ast::{Data, Fields, Style},
    FromDeriveInput, FromField, FromVariant,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{DeriveInput, Generics, Ident};

#[derive(Debug, FromField)]
struct StructLike {
    ident: Option<syn::Ident>,
    ty: syn::Type,
}

#[derive(Debug, FromVariant)]
struct EnumData {
    ident: syn::Ident,
    fields: Fields<StructLike>,
}

#[derive(Debug, FromDeriveInput)]
struct DeriveDiffable {
    ident: syn::Ident,
    data: Data<EnumData, StructLike>,
    generics: Generics,
}

fn diff_body(diff_ty: &Ident, variant_name: &Ident, fields: &Fields<StructLike>) -> TokenStream {
    let ident = idents(fields);

    let patch_ctor = match fields.style {
        Style::Tuple => quote! {
            // round brackets
            #diff_ty::#variant_name(
                # ( #ident, )*
            )
        },
        Style::Struct => quote! {
            // curly brackets
            #diff_ty::#variant_name {
                # ( #ident, )*
            }
        },
        Style::Unit => quote! {},
    };

    match fields.style {
        Style::Unit => quote! {
            // if unit-types match, by definition they are unchanged
            difficient::DeepDiff::Unchanged
        },
        Style::Tuple | Style::Struct => {
            let left_ident = prefixed_idents(fields, "left");
            let right_ident = prefixed_idents(fields, "right");
            quote! {
                #(
                    let #ident = #left_ident.diff(#right_ident);
                )*
                if #( #ident.is_unchanged() && )* true {
                    difficient::DeepDiff::Unchanged
                } else if #( #ident.is_replaced() && )* true {
                    difficient::DeepDiff::Replaced(other)
                } else {
                    difficient::DeepDiff::Patched(#patch_ctor)
                }
            }
        }
    }
}

impl DeriveDiffable {
    fn derive(&self) -> TokenStream {
        if !self.generics.params.is_empty() {
            panic!("derive(Diffable) does not support generic parameters")
        }

        let name = &self.ident;
        let diff_ty = format_ident!("{}Diff", self.ident);

        match &self.data {
            Data::Enum(variants) => {
                let var_name: Vec<&Ident> = variants.iter().map(|ed| &ed.ident).collect();
                let var_diff_def = variants.iter().map(|var| match var.fields.style {
                    Style::Unit => quote! {},
                    Style::Tuple => {
                        let ty = var.fields.iter().map(|data| &data.ty);
                        quote! {
                            (
                                #(  <#ty as difficient::Diffable<'a>>::Diff, )*
                            )
                        }
                    }
                    Style::Struct => {
                        let field = var
                            .fields
                            .iter()
                            .map(|data| &data.ident)
                            .collect::<Vec<_>>();
                        let ty = var.fields.iter().map(|data| &data.ty);
                        quote! {
                            {
                                #( #field: <#ty as difficient::Diffable<'a>>::Diff, )*
                            }
                        }
                    }
                });

                let enum_definition = quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    #[allow(dead_code)]
                    enum #diff_ty<'a> {
                        #(
                            #var_name #var_diff_def,
                        )*
                    }
                };

                let variant_diff_impl = variants.iter().zip(var_name.iter()).map(|(var, var_name)| {
                    let pattern_match_left = pattern_match(&var.fields, "left");
                    let pattern_match_right = pattern_match(&var.fields, "right");
                    let diff_impl = diff_body(&diff_ty, var_name, &var.fields);
                    quote! {
                        (Self::#var_name #pattern_match_left, Self::#var_name #pattern_match_right)  => {
                            #diff_impl
                        }
                    }
                });

                let diffable_impl = quote! {
                    impl<'a> difficient::Diffable<'a> for #name {
                        type Diff = difficient::DeepDiff<'a, Self, #diff_ty<'a>>;

                        fn diff(&self, other: &'a Self) -> Self::Diff {
                            use difficient::Replace as _;
                            match (self, other) {
                                #(
                                    #variant_diff_impl
                                ),*
                                _ => difficient::DeepDiff::Replaced(other)
                            }
                        }
                    }
                };

                let apply_body =variants.iter().zip(var_name.iter()).map(|(var, var_name)| {
                    let pat_l = prefixed_idents(&var.fields, "left");
                    let pat_r = prefixed_idents(&var.fields, "right");
                    let pattern_match_left = pattern_match(&var.fields, "left");
                    let pattern_match_right = pattern_match(&var.fields, "right");
                    quote! {
                        (Self::#var_name #pattern_match_left, #name::#var_name #pattern_match_right)  => {
                            #( #pat_l.apply_to_base(#pat_r, errs); )*
                        }
                    }
                }).collect::<Vec<_>>();

                let apply_impl = quote! {
                    impl<'a> difficient::Apply for #diff_ty<'a> {
                        type Parent = #name;
                        fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<difficient::ApplyError>) {
                            match (self, source) {
                                #( #apply_body )*
                                _ => errs.push(difficient::ApplyError::MismatchingEnum),
                            }
                        }
                    }
                };

                quote! {
                    #enum_definition

                    #diffable_impl

                    #apply_impl
                }
            }
            Data::Struct(fields) => {
                let ty = fields.iter().map(|data| &data.ty).collect::<Vec<_>>();
                if let Style::Unit = fields.style {
                    // short-circuit return
                    return quote! {
                        impl<'a> difficient::Diffable<'a> for #name {
                            type Diff = difficient::Id<Self>;

                            fn diff(&self, other: &'a Self) -> Self::Diff {
                                difficient::Id::new()
                            }
                        }
                    };
                };

                let field = idents(fields);
                let accessor = accessors(fields);
                let diff_ty_def = match fields.style {
                    Style::Tuple => {
                        quote! {
                            struct #diff_ty<'a>(
                                #(
                                    <#ty as difficient::Diffable<'a>>::Diff,
                                )*
                            );
                        }
                    }
                    Style::Struct => {
                        quote! {
                            struct #diff_ty<'a> {
                                #(
                                    #field: <#ty as difficient::Diffable<'a>>::Diff,
                                )*
                            }
                        }
                    }
                    Style::Unit => unreachable!(),
                };
                let patched_impl = match fields.style {
                    Style::Tuple => quote! {
                        #diff_ty( #( #field, )* )
                    },
                    Style::Struct => quote! {
                        #diff_ty{ #( #field ),* }
                    },
                    Style::Unit => unreachable!(),
                };
                quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    #diff_ty_def

                    impl<'a> difficient::Diffable<'a> for #name {
                        type Diff = difficient::DeepDiff<'a, Self, #diff_ty<'a>>;

                        fn diff(&self, other: &'a Self) -> Self::Diff {
                            use difficient::Replace as _;
                            #(
                                let #field = self.#accessor.diff(&other.#accessor);
                            )*
                            if #( #field.is_unchanged() && )* true {
                                difficient::DeepDiff::Unchanged
                            } else if #( #field.is_replaced() && )* true {
                                difficient::DeepDiff::Replaced(other)
                            } else {
                                difficient::DeepDiff::Patched(#patched_impl)
                            }
                        }
                    }

                    impl<'a> difficient::Apply for #diff_ty<'a> {
                        type Parent = #name;
                        fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<difficient::ApplyError>) {
                            #( self.#accessor.apply_to_base(&mut source.#accessor, errs) );*
                        }
                    }
                }
            }
        }
    }
}

impl ToTokens for DeriveDiffable {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.derive());
    }
}

fn pattern_match(fields: &Fields<StructLike>, prefix: &str) -> TokenStream {
    let pat = prefixed_idents(&fields, prefix);
    match fields.style {
        Style::Unit => quote! {},
        Style::Tuple => {
            quote! {
                (
                    #(  #pat, )*
                )
            }
        }
        Style::Struct => {
            let id = fields.iter().map(|data| &data.ident).collect::<Vec<_>>();
            quote! {
                {
                    #( #id: #pat, )*
                }
            }
        }
    }
}

fn prefixed_idents(fields: &Fields<StructLike>, prefix: &str) -> Vec<Ident> {
    fields
        .iter()
        .enumerate()
        .map(|(ix, sl)| {
            if let Some(field_name) = &sl.ident {
                format_ident!("{prefix}_{field_name}")
            } else {
                format_ident!("{prefix}_{ix}")
            }
        })
        .collect()
}

fn idents(fields: &Fields<StructLike>) -> Vec<Ident> {
    fields
        .iter()
        .enumerate()
        .map(|(ix, sl)| {
            if let Some(field_name) = &sl.ident {
                field_name.clone()
            } else {
                format_ident!("f{ix}")
            }
        })
        .collect()
}

fn accessors(fields: &Fields<StructLike>) -> Vec<TokenStream> {
    fields
        .iter()
        .enumerate()
        .map(|(ix, sl)| {
            if let Some(field_name) = &sl.ident {
                quote! { #field_name }
            } else {
                let ix = syn::Index::from(ix);
                quote! { #ix }
            }
        })
        .collect()
}

#[proc_macro_derive(Diffable)]
pub fn derive_diffable(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: DeriveInput = syn::parse(tokens).unwrap();
    let diff = DeriveDiffable::from_derive_input(&ast).unwrap();
    quote! { #diff }.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_struct() {
        let input = "
        #[derive(Diffable)]
        struct SimpleStruct {
            x: i32,
            y: String
        }
        ";

        let parsed = syn::parse_str(input).unwrap();
        let diff = DeriveDiffable::from_derive_input(&parsed).unwrap();
        let derived = quote! { #diff };

        let expect = quote! {
        #[derive(Debug, Clone, PartialEq)]
        struct SimpleStructDiff<'a> {
            x: <i32 as difficient::Diffable<'a>>::Diff,
            y: <String as difficient::Diffable<'a>>::Diff,
        }
        impl<'a> difficient::Diffable<'a> for SimpleStruct {
            type Diff = difficient::DeepDiff<'a, Self, SimpleStructDiff<'a>>;
            fn diff(&self, other: &'a Self) -> Self::Diff {
                use difficient::Replace as _;
                let x = self.x.diff(&other.x);
                let y = self.y.diff(&other.y);
                if x.is_unchanged() && y.is_unchanged() && true {
                    difficient::DeepDiff::Unchanged
                } else if x.is_replaced() && y.is_replaced() && true {
                    difficient::DeepDiff::Replaced(other)
                } else {
                    difficient::DeepDiff::Patched(SimpleStructDiff { x, y })
                }
            }
        }
        impl<'a> difficient::Apply for SimpleStructDiff<'a> {
            type Parent = SimpleStruct;
            fn apply_to_base(
                &self,
                source: &mut Self::Parent,
                errs: &mut Vec<difficient::ApplyError>
            ) {
                self.x.apply_to_base(&mut source.x, errs);
                self.y.apply_to_base(&mut source.y, errs)
            }
        }
        };

        assert_eq!(expect.to_string(), derived.to_string());
    }
}
