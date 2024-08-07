extern crate proc_macro;
use darling::{ast::Data, FromDeriveInput, FromField, FromVariant};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::DeriveInput;

#[derive(Debug, FromField)]
struct StructData {
    ident: Option<syn::Ident>,
    ty: syn::Type,
}

#[derive(Debug, FromVariant)]
struct EnumData {
    ident: syn::Ident,
}

#[derive(Debug, FromDeriveInput)]
struct DeriveDiffable {
    ident: syn::Ident,
    data: Data<EnumData, StructData>,
}

impl DeriveDiffable {
    fn derive(&self) -> TokenStream {
        let name = &self.ident;
        let diff_name = format_ident!("{}Diff", self.ident);

        match &self.data {
            Data::Enum(variants) => {
                todo!()
            }
            Data::Struct(fields) => {
                let field = fields.iter().map(|data| &data.ident).collect::<Vec<_>>();
                let ty = fields.iter().map(|data| &data.ty);
                quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    struct #diff_name {
                        #(
                            #field: <#ty as Diffable>::Diff,
                        )*
                    }

                    impl Diffable for #name {
                        type Diff = Diff<Self, #diff_name>;

                        fn diff(&self, other: &Self) -> Self::Diff {
                            #(
                                let #field = self.#field.diff(&other.#field);
                            )*
                            if #( #field.is_unchanged() && )* true {
                                Diff::Unchanged
                            } else if #( #field.is_replaced() && )* true {
                                # ( let #field = #field.get_replaced().unwrap(); )*
                                Diff::Replaced( Self { #( #field ),* })
                            } else {
                                Diff::Patched(#diff_name { #( #field ),* })
                            }
                        }
                    }

                    impl Apply for #diff_name {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_struct() {
        let input = "
        #[derive(Diffable)]
        struct Simple {
            x: i32,
            y: String
        }
        ";

        let parsed = syn::parse_str(input).unwrap();
        let diff = DeriveDiffable::from_derive_input(&parsed).unwrap();
        let derived = quote! { #diff };

        let expect = quote! {
            #[derive(Debug, Clone, PartialEq)]
            struct SimpleDiff {
                x: <i32 as Diffable>::Diff,
                y: <String as Diffable>::Diff,
            }
            impl Diffable for Simple {
                type Diff = Diff<Self, SimpleDiff>;
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
                        Diff::Patched(SimpleDiff { x, y })
                    }
                }
            }
            impl Apply for SimpleDiff {
                type Parent = Simple;
                fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                    self.x.apply(&mut source.x, errs);
                    self.y.apply(&mut source.y, errs)
                }
            }
        };

        assert_eq!(expect.to_string(), derived.to_string());
    }
}
