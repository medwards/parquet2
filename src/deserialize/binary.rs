use crate::{
    encoding::{hybrid_rle, plain_byte_array::BinaryIter},
    error::Error,
    page::{split_buffer, BinaryPageDict, DataPage},
    parquet_bridge::{Encoding, Repetition},
};

use super::utils;

#[derive(Debug)]
pub struct Dictionary<'a> {
    pub indexes: hybrid_rle::HybridRleDecoder<'a>,
    pub dict: &'a BinaryPageDict,
}

impl<'a> Dictionary<'a> {
    pub fn new(page: &'a DataPage, dict: &'a BinaryPageDict) -> Self {
        let indexes = utils::dict_indices_decoder(page);

        Self { indexes, dict }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.indexes.size_hint().0
    }
}

pub enum BinaryPageState<'a> {
    Optional(utils::DefLevelsDecoder<'a>, BinaryIter<'a>),
    Required(BinaryIter<'a>),
    RequiredDictionary(Dictionary<'a>),
    OptionalDictionary(utils::DefLevelsDecoder<'a>, Dictionary<'a>),
}

impl<'a> BinaryPageState<'a> {
    pub fn try_new(page: &'a DataPage) -> Result<Self, Error> {
        let is_optional =
            page.descriptor.primitive_type.field_info.repetition == Repetition::Optional;

        match (page.encoding(), page.dictionary_page(), is_optional) {
            (Encoding::PlainDictionary | Encoding::RleDictionary, Some(dict), false) => {
                let dict = dict.as_any().downcast_ref().unwrap();
                Ok(Self::RequiredDictionary(Dictionary::new(page, dict)))
            }
            (Encoding::PlainDictionary | Encoding::RleDictionary, Some(dict), true) => {
                let dict = dict.as_any().downcast_ref().unwrap();

                Ok(Self::OptionalDictionary(
                    utils::DefLevelsDecoder::new(page),
                    Dictionary::new(page, dict),
                ))
            }
            (Encoding::Plain, _, true) => {
                let (_, _, values) = split_buffer(page);

                let validity = utils::DefLevelsDecoder::new(page);
                let values = BinaryIter::new(values, None);

                Ok(Self::Optional(validity, values))
            }
            (Encoding::Plain, _, false) => {
                let (_, _, values) = split_buffer(page);
                let values = BinaryIter::new(values, Some(page.num_values()));

                Ok(Self::Required(values))
            }
            _ => Err(Error::General(format!(
                "Viewing page for encoding {:?} for binary type not supported",
                page.encoding(),
            ))),
        }
    }
}
