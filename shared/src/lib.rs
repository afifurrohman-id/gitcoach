use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Display, EnumIter, EnumString, Default)]
pub enum Persona {
    #[default]
    Cheerleader,
    TechLead,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Display, EnumIter, EnumString, Default)]
pub enum DomainGoal {
    #[default]
    FrontendWeb,
    BackendAPIs,
    SystemProgramming,
    MachineLearning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub persona: Persona,
    pub domain: DomainGoal,
    #[serde(default)]
    pub skill_tree: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatResponse {
    pub content: String,
    pub error: Option<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_skills: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_serialization() {
        let persona = Persona::Cheerleader;
        let json = serde_json::to_string(&persona).unwrap();
        assert_eq!(json, "\"Cheerleader\"");
    }

    #[test]
    fn test_domain_serialization() {
        let org = DomainGoal::BackendAPIs;
        let json = serde_json::to_string(&org).unwrap();
        assert_eq!(json, "\"BackendAPIs\"");
    }

    #[test]
    fn test_persona_display() {
        assert_eq!(Persona::TechLead.to_string(), "TechLead");
    }
}
