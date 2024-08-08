extern crate proc_macro;

use darling::{
    ast::{Data, Fields, Style},
    FromDeriveInput, FromField, FromVariant,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{DeriveInput, Ident};

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
            DeepDiff::Unchanged
        },
        Style::Tuple | Style::Struct => {
            let left_ident = prefixed_idents(fields, "left");
            let right_ident = prefixed_idents(fields, "right");
            let the_impl = deepdiff_impl(&ident, patch_ctor);
            quote! {
                #(
                    let #ident = #left_ident.diff(#right_ident);
                )*
                #the_impl
            }
        }
    }
}

fn deepdiff_impl(field: &[Ident], patch_impl: TokenStream) -> TokenStream {
    quote! {
        if #( #field.is_unchanged() && )* true {
            DeepDiff::Unchanged
        } else if #( #field.is_replaced() && )* true {
            DeepDiff::Replaced(other)
        } else {
            DeepDiff::Patched(#patch_impl)
        }
    }
}

impl DeriveDiffable {
    fn derive(&self) -> TokenStream {
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
                                #(  <#ty as Diffable<'a>>::Diff, )*
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
                                #( #field: <#ty as Diffable<'a>>::Diff, )*
                            }
                        }
                    }
                });

                let enum_definition = quote! {
                    #[derive(Debug, Clone, PartialEq)]
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
                    impl<'a> Diffable<'a> for #name {
                        type Diff = DeepDiff<'a, Self, #diff_ty<'a>>;

                        fn diff(&self, other: &'a Self) -> Self::Diff {
                            match (self, other) {
                                #(
                                    #variant_diff_impl
                                ),*
                                _ => DeepDiff::Replaced(other)
                            }
                        }
                    }
                };

                let apply_impl = quote! {
                    impl<'a> Apply for #diff_ty<'a> {
                        type Parent = #name;
                        fn apply_to_base(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                            todo!()
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
                        impl<'a> Diffable<'a> for #name {
                            type Diff = Id<Self>;

                            fn diff(&self, other: &'a Self) -> Self::Diff {
                                Id::new()
                            }
                        }
                    };
                };

                let field = idents(fields);
                let accessor = accessor(fields);
                let diff_ty_def = match fields.style {
                    Style::Tuple => {
                        quote! {
                            struct #diff_ty<'a>(
                                #(
                                    <#ty as Diffable<'a>>::Diff,
                                )*
                            );
                        }
                    }
                    Style::Struct => {
                        quote! {
                            struct #diff_ty<'a> {
                                #(
                                    #field: <#ty as Diffable<'a>>::Diff,
                                )*
                            }
                        }
                    }
                    Style::Unit => unreachable!(),
                };
                quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    #diff_ty_def

                    impl<'a> Diffable<'a> for #name {
                        type Diff = DeepDiff<'a, Self, #diff_ty<'a>>;

                        fn diff(&self, other: &'a Self) -> Self::Diff {
                            #(
                                let #field = self.#accessor.diff(&other.#accessor);
                            )*
                            if #( #field.is_unchanged() && )* true {
                                DeepDiff::Unchanged
                            } else if #( #field.is_replaced() && )* true {
                                DeepDiff::Replaced(other)
                            } else {
                                DeepDiff::Patched(todo!())
                            }
                        }
                    }

                    impl<'a> Apply for #diff_ty<'a> {
                        type Parent = #name;
                        fn apply_to_base(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
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

fn accessor(fields: &Fields<StructLike>) -> Vec<TokenStream> {
    fields
        .iter()
        .enumerate()
        .map(|(ix, sl)| {
            if let Some(field_name) = &sl.ident {
                quote! { #field_name }
            } else {
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
            struct SimpleStructDiff {
                x: <i32 as Diffable>::Diff,
                y: <String as Diffable>::Diff,
            }
            impl Diffable for SimpleStruct {
                type Diff = Diff<Self, SimpleStructDiff>;
                fn diff(&self, other: &Self) -> Self::Diff {
                    let x = self.x.diff(&other.x);
                    let y = self.y.diff(&other.y);
                    if x.is_unchanged() && y.is_unchanged() && true {
                        Diff::Unchanged
                    } else if x.is_replaced() && y.is_replaced() && true {
                        Diff::Replaced(other)
                    } else {
                        Diff::Patched(SimpleStructDiff { x, y })
                    }
                }
            }
            impl<'a> Apply for SimpleStructDiff {
                type Parent = SimpleStruct;
                fn apply_to_base(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                    self.x.apply(&mut source.x, errs);
                    self.y.apply(&mut source.y, errs)
                }
            }
        };

        assert_eq!(expect.to_string(), derived.to_string());
    }
}
