use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrequencyTuning {
    pub documents: HashMap<String, DocumentFrequency>,
    pub source_types: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFrequency {
    pub document_id: String,
    pub title: Option<String>,
    pub multiplier: f32,
    pub highlight_count: usize,
}

impl FrequencyTuning {
    pub fn get_document_multiplier(&self, document_id: &str) -> f32 {
        self.documents
            .get(document_id)
            .map(|d| d.multiplier)
            .unwrap_or(1.0)
    }

    pub fn get_source_type_multiplier(&self, source_type: &str) -> f32 {
        self.source_types.get(source_type).copied().unwrap_or(1.0)
    }

    pub fn get_combined_multiplier(&self, document_id: &str, source_type: &str) -> f32 {
        let doc_mult = self.get_document_multiplier(document_id);
        let source_mult = self.get_source_type_multiplier(source_type);
        doc_mult * source_mult
    }

    pub fn set_document(&mut self, document_id: String, multiplier: f32) {
        let multiplier = multiplier.clamp(0.1, 10.0);
        if let Some(doc) = self.documents.get_mut(&document_id) {
            doc.multiplier = multiplier;
        } else {
            self.documents.insert(
                document_id.clone(),
                DocumentFrequency {
                    document_id,
                    title: None,
                    multiplier,
                    highlight_count: 0,
                },
            );
        }
    }

    pub fn set_source_type(&mut self, source_type: String, multiplier: f32) {
        let multiplier = multiplier.clamp(0.1, 10.0);
        self.source_types.insert(source_type, multiplier);
    }

    pub fn remove_document(&mut self, document_id: &str) {
        self.documents.remove(document_id);
    }

    pub fn remove_source_type(&mut self, source_type: &str) {
        self.source_types.remove(source_type);
    }

    pub fn update_document_info(
        &mut self,
        document_id: String,
        title: Option<String>,
        highlight_count: usize,
    ) {
        if let Some(doc) = self.documents.get_mut(&document_id) {
            doc.title = title;
            doc.highlight_count = highlight_count;
        } else {
            self.documents.insert(
                document_id.clone(),
                DocumentFrequency {
                    document_id,
                    title,
                    multiplier: 1.0,
                    highlight_count,
                },
            );
        }
    }

    pub fn calculate_selection_probability(
        &self,
        document_id: &str,
        source_type: &str,
        total_highlights: usize,
        doc_highlights: usize,
    ) -> f64 {
        if total_highlights == 0 {
            return 0.0;
        }

        let base_prob = doc_highlights as f64 / total_highlights as f64;
        let multiplier = self.get_combined_multiplier(document_id, source_type) as f64;

        (base_prob * multiplier).min(1.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDocumentFrequencyRequest {
    pub document_id: String,
    pub multiplier: f32,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSourceTypeFrequencyRequest {
    pub source_type: String,
    pub multiplier: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_multipliers() {
        let tuning = FrequencyTuning::default();
        assert!((tuning.get_document_multiplier("doc1") - 1.0).abs() < 0.001);
        assert!((tuning.get_source_type_multiplier("book") - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_set_document_multiplier() {
        let mut tuning = FrequencyTuning::default();
        tuning.set_document("doc1".to_string(), 2.0);
        assert!((tuning.get_document_multiplier("doc1") - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_multiplier_clamping() {
        let mut tuning = FrequencyTuning::default();
        tuning.set_document("doc1".to_string(), 0.01);
        assert!((tuning.get_document_multiplier("doc1") - 0.1).abs() < 0.001);

        tuning.set_document("doc2".to_string(), 100.0);
        assert!((tuning.get_document_multiplier("doc2") - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_source_type_multiplier() {
        let mut tuning = FrequencyTuning::default();
        tuning.set_source_type("article".to_string(), 0.5);
        assert!((tuning.get_source_type_multiplier("article") - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_combined_multiplier() {
        let mut tuning = FrequencyTuning::default();
        tuning.set_document("doc1".to_string(), 2.0);
        tuning.set_source_type("book".to_string(), 1.5);

        let combined = tuning.get_combined_multiplier("doc1", "book");
        assert!((combined - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_selection_probability() {
        let mut tuning = FrequencyTuning::default();
        tuning.set_document("doc1".to_string(), 2.0);

        let prob = tuning.calculate_selection_probability("doc1", "book", 100, 10);
        assert!((prob - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_remove_document() {
        let mut tuning = FrequencyTuning::default();
        tuning.set_document("doc1".to_string(), 2.0);
        tuning.remove_document("doc1");
        assert!((tuning.get_document_multiplier("doc1") - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_update_document_info() {
        let mut tuning = FrequencyTuning::default();
        tuning.update_document_info("doc1".to_string(), Some("My Book".to_string()), 50);

        let doc = tuning.documents.get("doc1").unwrap();
        assert_eq!(doc.title, Some("My Book".to_string()));
        assert_eq!(doc.highlight_count, 50);
        assert!((doc.multiplier - 1.0).abs() < 0.001);
    }
}
