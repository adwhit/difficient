extern crate proc_macro;

use darling::{
    ast::{Data, Fields, Style},
    FromDeriveInput, FromField, FromVariant,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::DeriveInput;

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

impl DeriveDiffable {
    fn derive(&self) -> TokenStream {
        let name = &self.ident;
        let diff_name = format_ident!("{}Diff", self.ident);

        match &self.data {
            Data::Enum(variants) => {
                let var_name = variants.iter().map(|v| &v.ident).collect::<Vec<_>>();
                let var_diff_def = variants.iter().map(|v| match v.fields.style {
                    Style::Unit => quote! {},
                    Style::Tuple => {
                        let ty = v.fields.iter().map(|data| &data.ty);
                        quote! {
                            (
                                #(  <#ty as Diffable<'a>>::Diff, )*
                            )
                        }
                    }
                    Style::Struct => {
                        let field = v.fields.iter().map(|data| &data.ident).collect::<Vec<_>>();
                        let ty = v.fields.iter().map(|data| &data.ty);
                        quote! {
                            {
                                #( #field: <#ty as Diffable<'a>>::Diff, )*
                            }
                        }
                    }
                });
                let pattern_match_left = pattern_match(variants, "left");
                let pattern_match_right = pattern_match(variants, "right");
                quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    enum #diff_name<'a> {
                        #(
                            #var_name #var_diff_def,
                        )*
                    }

                    impl<'a> Diffable<'a> for #name {
                        type Diff = DeepDiff<'a, Self, #diff_name<'a>>;

                        fn diff(&self, other: &'a Self) -> Self::Diff {
                            match (self, other) {
                                #(
                                    (Self::#var_name #pattern_match_left, Self::#var_name #pattern_match_right)  => {
                                        todo!()
                                    }
                                ),*
                                _ => DeepDiff::Replaced(other)
                            }
                        }
                    }

                    impl<'a> Apply for #diff_name<'a> {
                        type Parent = #name;
                        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                            todo!()
                        }
                    }
                }
            }
            Data::Struct(fields) => {
                let field = fields.iter().map(|data| &data.ident).collect::<Vec<_>>();
                let ty = fields.iter().map(|data| &data.ty);
                quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    struct #diff_name<'a> {
                        #(
                            #field: <#ty as Diffable<'a>>::Diff,
                        )*
                    }

                    impl<'a> Diffable<'a> for #name {
                        type Diff = DeepDiff<'a, Self, #diff_name<'a>>;

                        fn diff(&self, other: &'a Self) -> Self::Diff {
                            #(
                                let #field = self.#field.diff(&other.#field);
                            )*
                            if #( #field.is_unchanged() && )* true {
                                DeepDiff::Unchanged
                            } else if #( #field.is_replaced() && )* true {
                                DeepDiff::Replaced(other)
                            } else {
                                DeepDiff::Patched(#diff_name { #( #field ),* })
                            }
                        }
                    }

                    impl<'a> Apply for #diff_name<'a> {
                        type Parent = #name;
                        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                            #( self.#field.apply(&mut source.#field, errs) );*
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

#[proc_macro_derive(Diffable)]
pub fn derive_diffable(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: DeriveInput = syn::parse(tokens).unwrap();
    let diff = DeriveDiffable::from_derive_input(&ast).unwrap();
    quote! { #diff }.into()
}

fn pattern_match(variants: &[EnumData], prefix: &str) -> Vec<TokenStream> {
    variants
        .iter()
        .map(|v| match v.fields.style {
            Style::Unit => quote! {},
            Style::Tuple => {
                let pat_l = v
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(ix, _)| format_ident!("{prefix}_{ix}"));
                quote! {
                    (
                        #(  #pat_l, )*
                    )
                }
            }
            Style::Struct => {
                let pat = v.fields.iter().map(|data| &data.ident).collect::<Vec<_>>();
                let pat_l = v
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(ix, _)| format_ident!("{prefix}_{ix}"));
                quote! {
                    {
                        #( #pat: #pat_l, )*
                    }
                }
            }
        })
        .collect()
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
                        let x = x.get_replaced().unwrap();
                        let y = y.get_replaced().unwrap();
                        Diff::Replaced(Self { x, y })
                    } else {
                        Diff::Patched(SimpleStructDiff { x, y })
                    }
                }
            }
            impl<'a> Apply for SimpleStructDiff {
                type Parent = SimpleStruct;
                fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                    self.x.apply(&mut source.x, errs);
                    self.y.apply(&mut source.y, errs)
                }
            }
        };

        assert_eq!(expect.to_string(), derived.to_string());
    }
}
