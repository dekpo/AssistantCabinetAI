//! A fake `OcrProvider` for tests, so extraction and indexing tests run without Tesseract
//! installed. Counts calls, which is the mechanism the tests in section M use to prove a
//! born-digital PDF invokes the provider zero times.

use std::sync::Mutex;

use super::{ImageFormat, OcrError, OcrInput, OcrPage, OcrProvider, OcrStatus};

pub struct FakeOcrProvider {
    calls: Mutex<Vec<(String, u32)>>,
    /// What `recognise` returns for a given `(relative_path, page_number)`. Falls back to
    /// `fallback` when the page was not configured.
    responses: Mutex<std::collections::HashMap<(String, u32), Result<OcrPage, OcrError>>>,
    fallback: Mutex<Result<OcrStatus, OcrError>>,
}

impl FakeOcrProvider {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(std::collections::HashMap::new()),
            fallback: Mutex::new(Ok(OcrStatus::Recognised)),
        }
    }

    /// Configures a specific answer for one page. Later calls to `recognise` for that
    /// `(relative_path, page_number)` return it verbatim.
    pub fn set_response(&self, relative_path: &str, page_number: u32, response: Result<OcrPage, OcrError>) {
        self.responses
            .lock()
            .expect("fake ocr lock")
            .insert((relative_path.to_string(), page_number), response);
    }

    /// Unconfigured pages return this status (with placeholder text for `Recognised`).
    pub fn set_fallback_status(&self, status: OcrStatus) {
        *self.fallback.lock().expect("fake ocr lock") = Ok(status);
    }

    /// Unconfigured pages return this error. `EngineUnavailable` is the degrade path.
    pub fn set_fallback_error(&self, error: OcrError) {
        *self.fallback.lock().expect("fake ocr lock") = Err(error);
    }

    /// A recognised page with the given text, for `set_response`.
    pub fn recognised_page(relative_path: &str, page_number: u32, text: &str) -> OcrPage {
        OcrPage {
            relative_path: relative_path.to_string(),
            page_number,
            text: text.to_string(),
            confidence: Some(0.9),
            engine: "fake".to_string(),
            engine_version: "0.0.0-test".to_string(),
            status: OcrStatus::Recognised,
        }
    }

    /// Every page ever passed to `recognise`, in call order. This is the count the "zero OCR
    /// calls on a born-digital PDF" test asserts on.
    pub fn calls(&self) -> Vec<(String, u32)> {
        self.calls.lock().expect("fake ocr lock").clone()
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().expect("fake ocr lock").len()
    }
}

impl Default for FakeOcrProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OcrProvider for FakeOcrProvider {
    fn id(&self) -> &str {
        "fake"
    }

    fn version(&self) -> &str {
        "0.0.0-test"
    }

    fn supports(&self, format: ImageFormat) -> bool {
        matches!(format, ImageFormat::Jpeg | ImageFormat::Png)
    }

    fn recognise(&self, input: &OcrInput<'_>) -> Result<OcrPage, OcrError> {
        self.calls
            .lock()
            .expect("fake ocr lock")
            .push((input.relative_path.to_string(), input.page_number));

        let key = (input.relative_path.to_string(), input.page_number);
        if let Some(configured) = self.responses.lock().expect("fake ocr lock").get(&key) {
            return configured.clone();
        }

        match self.fallback.lock().expect("fake ocr lock").clone() {
            Err(error) => Err(error),
            Ok(status) => Ok(OcrPage {
                relative_path: input.relative_path.to_string(),
                page_number: input.page_number,
                text: match status {
                    OcrStatus::Recognised => format!(
                        "recognised text for {} page {}",
                        input.relative_path, input.page_number
                    ),
                    OcrStatus::BelowThreshold => "low confidence text".to_string(),
                    OcrStatus::NoTextFound => String::new(),
                },
                confidence: match status {
                    OcrStatus::Recognised => Some(0.9),
                    OcrStatus::BelowThreshold => Some(0.2),
                    OcrStatus::NoTextFound => None,
                },
                engine: self.id().to_string(),
                engine_version: self.version().to_string(),
                status,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_are_recorded_in_order() {
        let provider = FakeOcrProvider::new();

        provider
            .recognise(&OcrInput {
                image: b"fake",
                format: ImageFormat::Png,
                relative_path: "scan.pdf",
                page_number: 1,
                locale: "fr-FR",
            })
            .expect("fake recognises");
        provider
            .recognise(&OcrInput {
                image: b"fake",
                format: ImageFormat::Png,
                relative_path: "scan.pdf",
                page_number: 2,
                locale: "fr-FR",
            })
            .expect("fake recognises");

        assert_eq!(
            provider.calls(),
            vec![("scan.pdf".to_string(), 1), ("scan.pdf".to_string(), 2)]
        );
        assert_eq!(provider.call_count(), 2);
    }

    #[test]
    fn a_configured_response_is_returned_verbatim() {
        let provider = FakeOcrProvider::new();
        provider.set_response("letter.pdf", 1, Err(OcrError::UnreadableImage));

        let result = provider.recognise(&OcrInput {
            image: b"fake",
            format: ImageFormat::Jpeg,
            relative_path: "letter.pdf",
            page_number: 1,
            locale: "fr-FR",
        });

        assert_eq!(result, Err(OcrError::UnreadableImage));
    }

    #[test]
    fn an_unconfigured_page_gets_a_default_recognised_answer() {
        let provider = FakeOcrProvider::new();

        let page = provider
            .recognise(&OcrInput {
                image: b"fake",
                format: ImageFormat::Png,
                relative_path: "letter.pdf",
                page_number: 1,
                locale: "fr-FR",
            })
            .expect("recognises");

        assert_eq!(page.status, OcrStatus::Recognised);
        assert_eq!(page.page_number, 1);
    }
}
