use darling::{ast, util, FromAttributes, FromDeriveInput, FromField, FromVariant};
use quote::{format_ident, ToTokens};
use syn::Ident;

#[derive(Debug)]
pub struct ReprTy(syn::Type);

impl FromAttributes for ReprTy {
    fn from_attributes(attrs: &[syn::Attribute]) -> darling::Result<Self> {
        let repr = attrs
            .iter()
            .find(|a| a.path().is_ident("repr"))
            .expect("Must have repr attribute");
        let ty: syn::Type = repr.parse_args()?;
        Ok(Self(ty))
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(
    forward_attrs(repr),
    attributes(pkt),
    supports(enum_named, enum_newtype, enum_tuple, enum_unit)
)]
pub struct ShroomPacketEnum {
    ident: Ident,
    data: ast::Data<ShroomEnumVariant, util::Ignored>,
    attrs: Vec<syn::Attribute>,
    //generics: syn::Generics,
}

#[derive(Debug, FromField)]
pub struct ShroomEnumField {
    ty: syn::Type,
}

#[derive(Debug, FromVariant)]
pub struct ShroomEnumVariant {
    ident: Ident,
    discriminant: Option<syn::Expr>,
    fields: ast::Fields<ShroomEnumField>,
}

impl ShroomEnumVariant {
    fn gen_encode_len(&self) -> proc_macro2::TokenStream {
        let ident = &self.ident;

        let fields = self.fields.iter().enumerate().map(|(i, _)| {
            let ident = format_ident!("_{i}");
            quote::quote! { #ident }
        });

        let field_plen = self.fields.iter().enumerate().map(|(i, _)| {
            let ident = format_ident!("_{i}");
            quote::quote! { #ident.encode_len() }
        });

        quote::quote! {
            Self::#ident(#(#fields,)*) => {
                #(#field_plen + )* 0
            }
        }
    }

    fn gen_decode(&self) -> proc_macro2::TokenStream {
        let ident = &self.ident;
        let discriminant = self
            .discriminant
            .as_ref()
            .expect("Must contain discriminant");

        let fields = self.fields.iter().enumerate().map(|(i, _)| {
            let ident = format_ident!("_{i}");
            quote::quote! { #ident }
        });

        let decode = self.fields.iter().enumerate().map(|(i, field)| {
            let ident = format_ident!("_{i}");
            let ty = &field.ty;
            quote::quote! { let #ident = <#ty>::decode(pr)?; }
        });

        quote::quote! {
            #discriminant => {
                #(#decode)*
                Self::#ident(#(#fields,)*)
            }
        }
    }

    fn gen_encode(&self) -> proc_macro2::TokenStream {
        let ident = &self.ident;

        let fields = self.fields.iter().enumerate().map(|(i, _)| {
            let ident = format_ident!("_{i}");
            quote::quote! { #ident }
        });

        let field_encode = self.fields.iter().enumerate().map(|(i, _)| {
            let ident = format_ident!("_{i}");
            quote::quote! { #ident.encode(pw)?; }
        });

        quote::quote! {
            Self::#ident(#(#fields,)*) => {
                #(#field_encode)*
            }
        }
    }
}

impl ToTokens for ShroomPacketEnum {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let repr = ReprTy::from_attributes(&self.attrs).expect("Must contain repr");

        let ident = self.ident.clone();
        let enum_ = self.data.as_ref().take_enum().expect("Must be enum");

        let enc_fields = enum_.iter().map(|v| v.gen_encode());
        let len_fields = enum_.iter().map(|v| v.gen_encode_len());
        let dec_fields = enum_.iter().map(|v| v.gen_decode());
        let repr_ty = &repr.0;
        tokens.extend(quote::quote!(
            impl shroom_pkt::EncodePacket for #ident {
                const SIZE_HINT: shroom_pkt::SizeHint = shroom_pkt::SizeHint::NONE;

                fn encode_len(&self) -> usize {
                    unsafe { *<*const _>::from(self).cast::<#repr_ty>() }.encode_len() +
                    match self {
                        #(#len_fields)*
                    }
                }

                fn encode<B: bytes::BufMut>(&self, pw: &mut shroom_pkt::PacketWriter<B>) ->  shroom_pkt::PacketResult<()> {
                    unsafe { *<*const _>::from(self).cast::<#repr_ty>() }.encode(pw)?;
                    match self {
                        #(#enc_fields)*
                    }
                    Ok(())
                }
            }

            impl<'de> shroom_pkt::DecodePacket<'de> for #ident {
                fn decode(pr: &mut shroom_pkt::PacketReader<'de>) -> shroom_pkt::PacketResult<Self> {
                    let disc = <#repr_ty>::decode(pr)?;
                    Ok(match disc {
                        #(#dec_fields)*
                        _ => return Err(shroom_pkt::Error::InvalidEnumDiscriminant(disc as usize))
                    })
                }
            }
        ));
    }
}
