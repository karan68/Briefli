use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_TEMPLATE_NAME_CHARS: usize = 120;
const MAX_TEMPLATE_DESCRIPTION_CHARS: usize = 500;
const MAX_TEMPLATE_SECTIONS: usize = 30;
const MAX_SECTION_TITLE_CHARS: usize = 120;
const MAX_SECTION_INSTRUCTION_CHARS: usize = 2_000;
const MAX_ITEM_FORMAT_CHARS: usize = 500;

fn char_count(value: &str) -> usize {
    value.chars().count()
}

/// Represents a single section in a meeting template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSection {
    /// Section title (e.g., "Summary", "Action Items")
    pub title: String,

    /// Instruction for the LLM on what to extract/include
    pub instruction: String,

    /// Format type: "paragraph", "list", or "string"
    pub format: String,

    /// Optional markdown formatting hint for list items (e.g., table structure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_format: Option<String>,

    /// Alternative formatting hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example_item_format: Option<String>,
}

/// Represents a complete meeting template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Template display name
    pub name: String,

    /// Brief description of the template's purpose
    pub description: String,

    /// List of sections in the template
    pub sections: Vec<TemplateSection>,
}

impl Template {
    /// Validates the template structure
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Template name cannot be empty".to_string());
        }
        if char_count(&self.name) > MAX_TEMPLATE_NAME_CHARS {
            return Err(format!(
                "Template name cannot exceed {} characters",
                MAX_TEMPLATE_NAME_CHARS
            ));
        }

        if self.description.trim().is_empty() {
            return Err("Template description cannot be empty".to_string());
        }
        if char_count(&self.description) > MAX_TEMPLATE_DESCRIPTION_CHARS {
            return Err(format!(
                "Template description cannot exceed {} characters",
                MAX_TEMPLATE_DESCRIPTION_CHARS
            ));
        }

        if self.sections.is_empty() {
            return Err("Template must have at least one section".to_string());
        }
        if self.sections.len() > MAX_TEMPLATE_SECTIONS {
            return Err(format!(
                "Template cannot have more than {} sections",
                MAX_TEMPLATE_SECTIONS
            ));
        }

        let mut section_titles = HashSet::new();
        for (i, section) in self.sections.iter().enumerate() {
            let title = section.title.trim();
            if title.is_empty() {
                return Err(format!("Section {} has empty title", i));
            }
            if char_count(&section.title) > MAX_SECTION_TITLE_CHARS {
                return Err(format!(
                    "Section '{}' title cannot exceed {} characters",
                    title, MAX_SECTION_TITLE_CHARS
                ));
            }
            if section.title.contains(['\r', '\n']) {
                return Err(format!("Section '{}' title cannot contain line breaks", title));
            }
            if !section_titles.insert(title.to_lowercase()) {
                return Err(format!("Section title '{}' is duplicated", title));
            }

            if section.instruction.trim().is_empty() {
                return Err(format!("Section '{}' has empty instruction", title));
            }
            if char_count(&section.instruction) > MAX_SECTION_INSTRUCTION_CHARS {
                return Err(format!(
                    "Section '{}' instruction cannot exceed {} characters",
                    title, MAX_SECTION_INSTRUCTION_CHARS
                ));
            }

            match section.format.as_str() {
                "paragraph" | "list" | "string" => {},
                other => return Err(format!(
                    "Section '{}' has invalid format '{}'. Must be 'paragraph', 'list', or 'string'",
                    section.title, other
                )),
            }

            for format_hint in [
                section.item_format.as_deref(),
                section.example_item_format.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                if char_count(format_hint) > MAX_ITEM_FORMAT_CHARS {
                    return Err(format!(
                        "Section '{}' item format cannot exceed {} characters",
                        title, MAX_ITEM_FORMAT_CHARS
                    ));
                }
            }
        }

        Ok(())
    }

    /// Generates a clean markdown template structure
    pub fn to_markdown_structure(&self) -> String {
        let mut markdown = String::from("# <Add Title here>\n\n");

        for section in &self.sections {
            markdown.push_str(&format!("**{}**\n\n", section.title));
        }

        markdown
    }

    /// Generates section-specific instructions for the LLM
    pub fn to_section_instructions(&self) -> String {
        let mut instructions = String::from(
            "- **For the main title (`# [AI-Generated Title]`):** Analyze the entire transcript and create a concise, descriptive title for the meeting.\n"
        );

        for section in &self.sections {
            instructions.push_str(&format!(
                "- **For the '{}' section:** {}.\n",
                section.title, section.instruction
            ));

            // Add item format instructions if present
            let item_format = section
                .item_format
                .as_ref()
                .or(section.example_item_format.as_ref());

            if let Some(format) = item_format {
                instructions.push_str(&format!(
                    "  - Items in this section should follow the format: `{}`.\n",
                    format
                ));
            }
        }

        instructions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_template() {
        let template = Template {
            name: "Test Template".to_string(),
            description: "A test template".to_string(),
            sections: vec![TemplateSection {
                title: "Summary".to_string(),
                instruction: "Provide a summary".to_string(),
                format: "paragraph".to_string(),
                item_format: None,
                example_item_format: None,
            }],
        };

        assert!(template.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_name() {
        let template = Template {
            name: "".to_string(),
            description: "A test template".to_string(),
            sections: vec![],
        };

        assert!(template.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_format() {
        let template = Template {
            name: "Test".to_string(),
            description: "Test".to_string(),
            sections: vec![TemplateSection {
                title: "Test".to_string(),
                instruction: "Test".to_string(),
                format: "invalid".to_string(),
                item_format: None,
                example_item_format: None,
            }],
        };

        assert!(template.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_whitespace_and_duplicate_titles() {
        let mut template = Template {
            name: "Test".to_string(),
            description: "Test".to_string(),
            sections: vec![
                TemplateSection {
                    title: "Summary".to_string(),
                    instruction: "First".to_string(),
                    format: "paragraph".to_string(),
                    item_format: None,
                    example_item_format: None,
                },
                TemplateSection {
                    title: " summary ".to_string(),
                    instruction: "Second".to_string(),
                    format: "paragraph".to_string(),
                    item_format: None,
                    example_item_format: None,
                },
            ],
        };

        assert!(template.validate().unwrap_err().contains("duplicated"));

        template.sections.truncate(1);
        template.sections[0].instruction = "   ".to_string();
        assert!(template.validate().unwrap_err().contains("empty instruction"));
    }

    #[test]
    fn test_validate_rejects_excessive_content() {
        let template = Template {
            name: "Test".to_string(),
            description: "Test".to_string(),
            sections: vec![TemplateSection {
                title: "Summary".to_string(),
                instruction: "x".repeat(MAX_SECTION_INSTRUCTION_CHARS + 1),
                format: "paragraph".to_string(),
                item_format: None,
                example_item_format: None,
            }],
        };

        assert!(template.validate().unwrap_err().contains("cannot exceed"));
    }
}
