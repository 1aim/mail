use failure::Error;
use hbs;
use serde::Serialize;

use super::{
    serde_impl, AdditionalCIds, BodyTemplate, TemplateEngine, TemplateEngineCanHandleData,
};

//TODO[FEAT] add custom engine config section to loading
// e.g. something like:
// ```
// [engine]
// load_partial = "../partials/baseh.html"
// ```
//
// Just specific to each engine.

pub struct Handlebars {
    inner: hbs::Handlebars,
    name_counter: usize,
}

impl Handlebars {
    pub fn new() -> Self {
        Handlebars {
            inner: hbs::Handlebars::new(),
            name_counter: 0,
        }
    }

    pub fn inner(&self) -> &hbs::Handlebars {
        &self.inner
    }

    /// Provides mutable access to the underling handlebars instance.
    ///
    /// This can be used to e.g. add partials (in the future the template
    /// file will have a custom config section but currently it doesn't).
    pub fn inner_mut(&mut self) -> &mut hbs::Handlebars {
        &mut self.inner
    }

    fn next_body_template_name(&mut self) -> String {
        let name = format!("body_{}", self.name_counter);
        self.name_counter += 1;
        name
    }
}
impl TemplateEngine for Handlebars {
    type Id = String;

    type LazyBodyTemplate = serde_impl::StandardLazyBodyTemplate;

    fn load_body_template(
        &mut self,
        tmpl: Self::LazyBodyTemplate,
    ) -> Result<BodyTemplate<Self>, Error> {
        let serde_impl::StandardLazyBodyTemplate {
            path,
            embeddings,
            media_type,
        } = tmpl;

        let name = self.next_body_template_name();
        self.inner.register_template_file(&name, &path)?;

        const ERR_BAD_MEDIA_TYPE_DETECTION: &str =
            "handlebars requires html/txt file extension or media type given in template spec";

        let media_type = if let Some(media_type) = media_type {
            media_type
        } else if let Some(extension) = path.extension().and_then(|osstr| osstr.to_str()) {
            match extension {
                "html" => "text/html; charset=utf-8".parse().unwrap(),
                "txt" => "text/plain; charset=utf-8".parse().unwrap(),
                _ => {
                    return Err(failure::err_msg(ERR_BAD_MEDIA_TYPE_DETECTION));
                }
            }
        } else {
            return Err(failure::err_msg(ERR_BAD_MEDIA_TYPE_DETECTION));
        };

        Ok(BodyTemplate {
            template_id: name,
            media_type,
            inline_embeddings: embeddings,
        })
    }

    fn load_subject_template(&mut self, template_string: String) -> Result<Self::Id, Error> {
        let id = "subject".to_owned();
        self.inner.register_template_string(&id, template_string)?;
        Ok(id)
    }
}

/// Additional trait a template engine needs to implement for the types it can process as input.
///
/// This could for example be implemented in a wild card impl for the template engine for
/// any data `D` which implements `Serialize`.
impl<D> TemplateEngineCanHandleData<D> for Handlebars
where
    D: Serialize,
{
    fn render<'r, 'a>(
        &'r self,
        id: &'r Self::Id,
        data: &'r D,
        additional_cids: AdditionalCIds<'r>,
    ) -> Result<String, Error> {
        Ok(self.inner.render(
            id,
            &SerHelper {
                data,
                cids: additional_cids,
            },
        )?)
    }
}

#[derive(Serialize)]
struct SerHelper<'r, D: 'r> {
    data: &'r D,
    cids: AdditionalCIds<'r>,
}
