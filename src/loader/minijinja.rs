use fluent_bundle::FluentValue;
use minijinja::value::DynObject;
use minijinja::value::Kwargs;
use minijinja::value::Value;
//use serde_json::Value as Json;
use std::borrow::Cow;
use std::collections::HashMap;
use unic_langid::LanguageIdentifier;

use crate::Loader;

const LANG_KEY: &str = "lang";
//const FLUENT_KEY: &str = "key";

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("No `lang` argument provided.")]
    NoLangArgument,
    #[error("`lang` must be a valid unicode language identifier.")]
    LangArgumentInvalid,
    #[error("Couldn't convert minijinja::Value to Fluent value.")]
    ValueToFluentFail,
}

impl From<Error> for minijinja::Error {
    fn from(error: Error) -> Self {
        minijinja::Error::new(minijinja::ErrorKind::UndefinedError, error.to_string())
    }
}

fn value_to_fluent(value: &Value) -> crate::Result<FluentValue<'static>, minijinja::Error> {
    match value {
        v if v.is_integer() => Ok(FluentValue::from(i64::try_from(v.clone())?)),
        v if v.is_number() => Ok(FluentValue::from(f64::try_from(v.clone())?)),
        v => {
            if let Some(s) = v.as_str() {
                Ok(FluentValue::from(s.to_string()))
            } else {
                Err(Error::ValueToFluentFail)?
            }
        }
    }
}

fn parse_language(arg: &str) -> crate::Result<LanguageIdentifier, Error> {
    arg.parse::<LanguageIdentifier>()
        .ok()
        .ok_or(Error::LangArgumentInvalid)
}

impl<L: Loader + Send + Sync> crate::FluentLoader<L> {
    fn minijinja_call(
        &self,
        id: String,
        object: Option<Value>,
        kwargs: Kwargs,
    ) -> Result<String, minijinja::Error> {
        let lang_arg = kwargs.get(LANG_KEY).ok().map(parse_language).transpose()?;
        let lang = lang_arg
            .as_ref()
            .or(self.default_lang.as_ref())
            .ok_or(Error::NoLangArgument)?;

        /// Filters kwargs to exclude ones used by this function and tera.
        fn is_not_tera_key(k: &str) -> bool {
            k != LANG_KEY
        }

        let mut map = HashMap::new();

        if let Some(o) = object.as_ref() {
            for i in o.try_iter()? {
                map.insert(
                    i.as_str().unwrap().to_string(),
                    value_to_fluent(&o.get_item(&i)?)?,
                );
            }
        }
        for key in kwargs.args() {
            let value = &kwargs.get(key)?;
            map.insert(key.to_string(), value_to_fluent(value)?);
        }

        let fluent_args: HashMap<_, _> = map
            .into_iter()
            .filter(|(k, v)| is_not_tera_key(k))
            .map(|(k, v)| (Cow::from(heck::ToKebabCase::to_kebab_case(k.as_str())), v))
            .collect();

        let response = self.loader.lookup_with_args(lang, &id, &fluent_args);
        Ok(response)
    }
    pub fn into_minijinja_fn(
        self,
    ) -> impl Fn(String, Option<Value>, Kwargs) -> Result<String, minijinja::Error> {
        move |a, b, c| self.minijinja_call(a, b, c)
    }
}
