use axum::{
    extract::State,
    http::Method,
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use gemini_rust::{
    prelude::*, GeminiBuilder,
    Message as GeminiMessage,
};
use reqwest::Client as ReqwestClient;
use shared::{ChatRequest, ChatResponse, DomainGoal, Persona, Role};
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

pub struct AppState {
    client: Gemini,
    // Warning: In a real app, you'd want a session per user.
    // This simple example shares one chat history across all users.
    chat_history: Mutex<Vec<Content>>,
}

pub fn app(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(|| async { "OK" }))
        .route("/api/chat", post(handle_chat))
        .layer(cors)
        .with_state(state)
}

#[tokio::main]
async fn main() {
    // Load .env file
    dotenv().ok();

    // Initialize tracing (optional but good for debugging)
    tracing_subscriber::fmt::init();

    // Initialize Gemini Client
    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set");
    let client = GeminiBuilder::new(&api_key).with_model(Model::Gemini25Flash).build().expect("Failed to build Gemini client");
    
    let chat_history = Mutex::new(Vec::new());

    let state = Arc::new(AppState {
        client,
        chat_history,
    });

    let app = app(state);

    // Run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Backend server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

fn get_github_language_query(domain: &DomainGoal) -> &'static str {
    match domain {
        DomainGoal::FrontendWeb => "javascript",
        DomainGoal::BackendAPIs => "go",
        DomainGoal::SystemProgramming => "rust",
        DomainGoal::MachineLearning => "python",
    }
}

async fn fetch_github_issues(domain: &DomainGoal) -> String {
    let client = ReqwestClient::new();
    let lang_query = get_github_language_query(domain);
    let domain_str = domain.to_string();
    
    let url = format!(
        "https://api.github.com/search/issues?q=is:issue+is:open+label:\"good first issue\"+language:{}&per_page=3",
        lang_query
    );


    println!("Fetching github issues: {}", url);

    let res = client.get(&url)
        .header("User-Agent", "GitCoach-AI-Mentor-v1.0")
        .send()
        .await;
        
    match res {
        Ok(response) if response.status().is_success() => {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
                    let mut issue_texts = Vec::new();
                    for item in items.iter().take(3) {
                        let title = item.get("title").and_then(|t| t.as_str()).unwrap_or("Unknown Title");
                        let html_url = item.get("html_url").and_then(|h| h.as_str()).unwrap_or("");
                        issue_texts.push(format!("- [**{}**]({})", title, html_url));
                    }
                    if !issue_texts.is_empty() {
                        return format!("Here are some live open 'good first issues' for {} (Language: {}):\n{}", domain_str, lang_query, issue_texts.join("\n"));
                    }
                }
            }
            "No open issues found right now.".to_string()
        },
        _ => "Failed to fetch issues from GitHub.".to_string(),
    }
}

async fn fetch_repo_contributing_md(repo: &str) -> String {
    let client = ReqwestClient::new();
    // Try CONTRIBUTING.md first
    let url = format!("https://raw.githubusercontent.com/{}/HEAD/CONTRIBUTING.md", repo);
    let res = client.get(&url).send().await;

    if let Ok(response) = res {
        if response.status().is_success() {
            if let Ok(text) = response.text().await {
                return format!("CONTRIBUTING.md found:\n{}", text.chars().take(3000).collect::<String>()); // cap at 3000 chars to save context
            }
        }
    }
    
    // Fallback to README
    let fallback_url = format!("https://api.github.com/repos/{}/readme", repo);
    if let Ok(response) = client.get(&fallback_url).header("User-Agent", "GitCoach").header("Accept", "application/vnd.github.v3.raw").send().await {
        if response.status().is_success() {
            if let Ok(text) = response.text().await {
                return format!("No contributing.md found. README found:\n{}", text.chars().take(3000).collect::<String>());
            }
        }
    }
    
    "Failed to fetch repository documentation.".to_string()
}

pub fn generate_system_context(persona: &Persona, domain: &DomainGoal, skill_tree: &[String], issues_context: &str) -> String {
    let persona_instruction = match persona {
        Persona::Cheerleader => "You are an incredibly encouraging open-source mentor. You break things down into simple 1-line steps, use lots of emojis, and fight imposter syndrome actively.",
        Persona::TechLead => "You are a extremely strict, highly professional Tech Lead. You expect the user to read the docs, aggressively demand unit tests, and keep your answers brief, terse, and serious. No emojis.",
    };

    let user_skills = if skill_tree.is_empty() {
        "The user is a beginner and has no registered skills yet.".to_string()
    } else {
        format!("The user has already mastered the following concepts: {}. Do not over-explain these concepts.", skill_tree.join(", "))
    };

    format!(
        "SYSTEM INSTRUCTION (Adopt this persona completely): {}\n\n\
        The user's domain goal is {}. {}\n\n{}\n\n\
        CRITICAL OUTPUT FORMAT: You are a strict JSON-only API. You MUST return your response as a single, valid JSON object.\n\
        IMPORTANT MARKDOWN RULES: \n\
        - Format your `content` using standard markdown.\n\
        - ALWAYS add an empty blank line before AND after any list. Without this, lists will break.\n\
        - When separating numbered lists with paragraphs, always restart numbering at `1.` or use `-` bullet points. Otherwise markdown parsers will inline them.\n\
        The JSON must match this exact structure:\n\
        {{\n\
          \"content\": \"Your actual markdown message to the user.\",\n\
          \"suggestions\": [\"A short suggested follow-up question the user could click\", \"Another follow-up\"],\n\
          \"new_skills_detected\": [\"Any NEW core concepts the user demonstrated understanding of in their last message (Optional)\"],\n\
          \"fetch_repo_rag\": \"If they asked how to start contributing to a specific repository, put the 'owner/repo' here so the backend can fetch the CONTRIBUTING.md. Otherwise leave null.\"\n\
        }}", 
        persona_instruction, 
        domain.to_string(),
        user_skills,
        if !issues_context.is_empty() {
            format!("Live GitHub issues you MUST recommend and link to the user:\n{}", issues_context)
        } else {
            String::new()
        }
    )
}

async fn handle_chat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatRequest>,
) -> Json<ChatResponse> {
    // Extract the latest user message
    let last_user_message = match payload.messages.last() {
        Some(msg) if msg.role == Role::User => &msg.content,
        _ => {
            return Json(ChatResponse {
                content: String::new(),
                error: Some("No user message provided".to_string()),
                ..Default::default()
            });
        }
    };

    let mut history = state.chat_history.lock().await;

    // Append user message to history
    let user_content = GeminiMessage::user(last_user_message).content;
    history.push(user_content);    // Inject system prompt and API data
    let needs_recommendations = history.len() == 1 || last_user_message.to_lowercase().contains("recommend") || last_user_message.to_lowercase().contains("issue") || last_user_message.to_lowercase().contains("find");
    let issues_context = if needs_recommendations {
        fetch_github_issues(&payload.domain).await
    } else {
        String::new()
    };
    
    let system_context = generate_system_context(&payload.persona, &payload.domain, &payload.skill_tree, &issues_context);

    let result = tokio::time::timeout(std::time::Duration::from_secs(5 * 60), execute_rag_agent(&state.client, &mut history, &system_context, 0)).await;
    
    match result {
        Ok(response_json) => response_json,
        Err(_) => {
            history.pop(); // Remove the unresolved user message so they can retry cleanly
            Json(ChatResponse {
                content: String::new(),
                error: Some("The request timed out. The AI took too long to respond. Please try again.".to_string()),
                ..Default::default()
            })
        }
    }
}

use std::pin::Pin;
use std::future::Future;

fn execute_rag_agent<'a>(
    client: &'a gemini_rust::Gemini,
    history: &'a mut Vec<gemini_rust::Content>,
    system_context: &'a str,
    depth: u8,
) -> Pin<Box<dyn Future<Output = Json<ChatResponse>> + Send + 'a>> {
    Box::pin(async move {
        if depth > 3 {
            return Json(ChatResponse {
                content: String::new(),
                error: Some("Agent exceeded maximum iterations".to_string()),
                ..Default::default()
            });
        }

        let mut builder = client.generate_content();
        
        let config = gemini_rust::prelude::GenerationConfig {
            response_mime_type: Some("application/json".to_string()),
            max_output_tokens: Some(8192),
            ..Default::default()
        };
        builder = builder.with_generation_config(config);

        // Inject system prompt (pseudo messages)
        builder.contents.push(GeminiMessage::user(system_context).content);
        builder.contents.push(GeminiMessage::model("{\n  \"content\": \"Understood. I will adopt this persona exclusively, use the provided API data, assume the provided skills, and ensure every response is raw JSON matching the schema. If the user asks how to contribute to a specific repository, I will provide the owner/repo in fetch_repo_rag.\",\n  \"suggestions\": [],\n  \"new_skills_detected\": [],\n  \"fetch_repo_rag\": null\n}").content);

        // Inject all historical messages
        for content in history.iter() {
            let cloned_content = content.clone();
            let role = cloned_content.role.clone().unwrap_or(gemini_rust::Role::User);
            builder.contents.push(cloned_content.with_role(role));
        }

        match builder.execute().await {
        Ok(response) => {
            // Extract text from the primary response part
            let raw_content = if let Some(candidate) = response.candidates.first() {
                if let Some(parts) = candidate.content.parts.as_ref() {
                    if let Some(part) = parts.first() {
                        match part {
                            gemini_rust::Part::Text { text, .. } => text.clone(),
                            _ => "Received non-text response".to_string(),
                        }
                    } else {
                        "Empty response parts".to_string()
                    }
                } else {
                    "Empty response parts".to_string()
                }
            } else {
                "No candidates returned".to_string()
            };

            // Extract JSON block in case LLM output conversational text outside of it
            let extracted_json = if let Some(start) = raw_content.find('{') {
                if let Some(end) = raw_content.rfind('}') {
                    raw_content[start..=end].to_string()
                } else {
                    raw_content
                }
            } else {
                raw_content
            };

            // Clean up potentially malformed JSON fences from LLM
            let cleaned_json: String = extracted_json
                .lines()
                .filter(|line| !line.starts_with("```"))
                .collect::<Vec<_>>()
                .join("\n");

            #[derive(serde::Deserialize)]
            struct AiJsonResponse {
                content: String,
                suggestions: Option<Vec<String>>,
                new_skills_detected: Option<Vec<String>>,
                fetch_repo_rag: Option<String>,
            }

            match serde_json::from_str::<AiJsonResponse>(&cleaned_json) {
                Ok(parsed) => {
                    // Check if the agent wants to perform a tool call (RAG)
                    if let Some(repo) = parsed.fetch_repo_rag {
                        if !repo.is_empty() && repo != "null" {
                            println!("[RAG]: Fetching repository for {}", repo);

                            let rag_context = fetch_repo_contributing_md(&repo).await;
                            
                            // Append intermediate tool call to history so it knows what it fetched
                            history.push(GeminiMessage::model(cleaned_json.clone()).content);
                            history.push(GeminiMessage::user(&format!("SYSTEM TOOL FETCH RESULT for {}:\n{}", repo, rag_context)).content);
                            
                            // Recurse to get the final answer built on the RAG context!
                            return execute_rag_agent(client, history, system_context, depth + 1).await;
                        }
                    }

                    // Append the actual final response to history
                    let assistant_content = GeminiMessage::model(parsed.content.clone()).content;
                    history.push(assistant_content);

                    Json(ChatResponse {
                        content: parsed.content,
                        error: None,
                        suggestions: parsed.suggestions.unwrap_or_default(),
                        new_skills: parsed.new_skills_detected,
                    })
                }
                Err(e) => {
                    // Fallback if LLM broke schema (e.g. raw newlines in string, missing prefix)
                    eprintln!("Failed to parse structured JSON: {}. Attempting manual extraction...", e);
                    
                    let (fallback_content, fallback_suggestions, fallback_rag) = fallback_parse_json(&cleaned_json);

                    if let Some(repo) = fallback_rag {
                        println!("[RAG Fallback]: Fetching repository for {}", repo);
                        let rag_context = fetch_repo_contributing_md(&repo).await;
                        // Append intermediate tool call to history
                        history.push(GeminiMessage::model(cleaned_json.clone()).content);
                        history.push(GeminiMessage::user(&format!("SYSTEM TOOL FETCH RESULT for {}:\n{}", repo, rag_context)).content);
                        return execute_rag_agent(client, history, system_context, depth + 1).await;
                    }

                    let assistant_content = GeminiMessage::model(fallback_content.clone()).content;
                    history.push(assistant_content);
                    
                    Json(ChatResponse {
                        content: fallback_content,
                        error: None,
                        suggestions: fallback_suggestions,
                        new_skills: None,
                    })
                }
            }
        }
        Err(e) => {
            let error_msg = format!("{:?}", e);
            eprintln!("Gemini API Error: {}", error_msg);

            let user_friendly_error = parse_gemini_error(&error_msg);

            Json(ChatResponse {
                content: String::new(),
                error: Some(user_friendly_error),
                ..Default::default()
            })
        }
    }
    })
}

pub fn fallback_parse_json(cleaned_json: &str) -> (String, Vec<String>, Option<String>) {
    let mut fallback_content = cleaned_json.to_string();
    let mut fallback_suggestions = Vec::new();
    let mut fallback_rag = None;

    // 1. Manually extract content block
    let content_end_idx = cleaned_json.find("\"suggestions\"").unwrap_or(cleaned_json.len());
    let content_part = &cleaned_json[..content_end_idx];
    
    if let Some(content_start) = content_part.find("\"content\"") {
        if let Some(colon_idx) = content_part[content_start..].find(':') {
            let actual_start = content_start + colon_idx + 1;
            let clean = content_part[actual_start..]
                .trim()
                .trim_start_matches('"')
                .trim_end_matches(',')
                .trim_end_matches('\n')
                .trim_end_matches('"');
            fallback_content = clean.to_string();
            fallback_content = fallback_content.replace("\\n", "\n").replace("\\\"", "\"");
        }
    } else {
        // If it doesn't even have "content", maybe it just output raw text?
        fallback_content = content_part.to_string();
    }

    // 2. Manually extract suggestions
    if let Some(sugg_start) = cleaned_json.find("\"suggestions\"") {
        if let Some(sugg_end) = cleaned_json[sugg_start..].find(']') {
            if let Some(ob) = cleaned_json[sugg_start..sugg_start+sugg_end].find('[') {
                let sugg_block = &cleaned_json[sugg_start + ob + 1 .. sugg_start + sugg_end];
                for line in sugg_block.split(',') {
                    let s = line.trim().trim_matches('"').trim();
                    if !s.is_empty() {
                        fallback_suggestions.push(s.to_string());
                    }
                }
            }
        }
    }

    // 3. Manually extract RAG
    if let Some(rag_start) = cleaned_json.find("\"fetch_repo_rag\"") {
        let rag_part = &cleaned_json[rag_start..];
        if let Some(colon_idx) = rag_part.find(':') {
            let after_colon = rag_part[colon_idx+1..].trim();
            if after_colon.starts_with('"') {
                if let Some(end_quote) = after_colon[1..].find('"') {
                    let rag = &after_colon[1..1+end_quote];
                    if !rag.is_empty() && rag != "null" {
                        fallback_rag = Some(rag.to_string());
                    }
                }
            }
        }
    }

    (fallback_content, fallback_suggestions, fallback_rag)
}

// Extracted for unit testing
pub fn parse_gemini_error(error_msg: &str) -> String {
    if error_msg.contains("429") || error_msg.contains("RESOURCE_EXHAUSTED") || error_msg.contains("Quota") {
        "Woops! It looks like we've hit the free tier quota limit for the Google Gemini API. Please wait a minute and try again!".to_string()
    } else if error_msg.contains("503") || error_msg.contains("UNAVAILABLE") {
        "The Gemini API is currently experiencing extremely high demand. Please wait a few moments and try sending your message again!".to_string()
    } else {
        format!("Failed to communicate with AI: {}", error_msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use axum::body::Body;
    use tower::ServiceExt;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        // We can pass a dummy API key since health doesn't use the client
        let client = GeminiBuilder::new("TEST_KEY").build().unwrap();
        let state = Arc::new(AppState {
            client,
            chat_history: Mutex::new(Vec::new()),
        });

        let app = app(state);

        let response = app
            .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), 200);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"OK");
    }

    #[tokio::test]
    async fn test_chat_empty_message() {
        let client = GeminiBuilder::new("TEST_KEY").build().unwrap();
        let state = Arc::new(AppState {
            client,
            chat_history: Mutex::new(Vec::new()),
        });

        let app = app(state);

        let empty_payload = serde_json::to_vec(&ChatRequest {
            messages: vec![],
            persona: shared::Persona::Cheerleader,
            domain: shared::DomainGoal::SystemProgramming,
            skill_tree: vec![],
        }).unwrap();

        let request = Request::builder()
            .uri("/api/chat")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(empty_payload))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), 200); // We return a 200 with error payload
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        
        let chat_response: ChatResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(chat_response.error.is_some());
        assert_eq!(chat_response.error.unwrap(), "No user message provided");
    }

    #[test]
    fn test_generate_system_context_cheerleader_rust() {
        let skill_tree = vec!["Borrow Checking".to_string()];
        let context = generate_system_context(
            &Persona::Cheerleader, 
            &DomainGoal::SystemProgramming, 
            &skill_tree,
            "Test issue: fix typo in README"
        );
        assert!(context.contains("encouraging open-source mentor"));
        assert!(context.contains("SystemProgramming"));
        assert!(context.contains("fix typo in README"));
        assert!(context.contains("Borrow Checking"));
        assert!(context.contains("valid JSON object"));
    }

    #[test]
    fn test_generate_system_context_techlead_no_issues() {
        let context = generate_system_context(
            &Persona::TechLead, 
            &DomainGoal::BackendAPIs, 
            &[],
            ""
        );
        assert!(context.contains("strict, highly professional Tech Lead"));
        assert!(context.contains("BackendAPIs"));
        assert!(!context.contains("Live GitHub issues"));
        assert!(context.contains("JSON"));
    }

    #[test]
    fn test_ai_json_response_deserialization() {
        let json_str = r#"{
            "content": "You just mastered lifetimes!",
            "suggestions": ["What about closures?"],
            "new_skills_detected": ["Rust Lifetimes"],
            "fetch_repo_rag": null
        }"#;

        #[derive(serde::Deserialize)]
        struct AiJsonResponse {
            content: String,
            suggestions: Option<Vec<String>>,
            new_skills_detected: Option<Vec<String>>,
            fetch_repo_rag: Option<String>,
        }

        let parsed: AiJsonResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed.content, "You just mastered lifetimes!");
        assert_eq!(parsed.suggestions.unwrap()[0], "What about closures?");
        assert_eq!(parsed.new_skills_detected.unwrap()[0], "Rust Lifetimes");
        assert!(parsed.fetch_repo_rag.is_none());
    }

    #[test]
    fn test_parse_gemini_error_rate_limit() {
        let raw_429_error = r#"bad response from server; code 429; description: { "error": { "code": 429, "message": "You exceeded your current quota..." } }"#;
        let parsed_error = parse_gemini_error(raw_429_error);
        assert_eq!(parsed_error, "Woops! It looks like we've hit the free tier quota limit for the Google Gemini API. Please wait a minute and try again!");

        let raw_quota_error = "RESOURCE_EXHAUSTED";
        let parsed_error2 = parse_gemini_error(raw_quota_error);
        assert_eq!(parsed_error2, "Woops! It looks like we've hit the free tier quota limit for the Google Gemini API. Please wait a minute and try again!");

        let raw_503_error = r#"bad response from server; code 503; description: "UNAVAILABLE""#;
        let parsed_error3 = parse_gemini_error(raw_503_error);
        assert_eq!(parsed_error3, "The Gemini API is currently experiencing extremely high demand. Please wait a few moments and try sending your message again!");

        let generic_error = "500 Internal Server Error";
        let parsed_error4 = parse_gemini_error(generic_error);
        assert_eq!(parsed_error4, "Failed to communicate with AI: 500 Internal Server Error");
    }

    #[test]
    fn test_fallback_parse_json() {
        let raw = r#""YES! 🙌 That's an absolutely fantastic choice! Getting involved with `Implement API V2 analysis endpoints` is an awesome way to learn about building API clients and making real-world contributions. You are already doing so well! Don't let that little voice of doubt sneak in – you're totally capable of tackling this! 💪

Here’s your super simple roadmap to getting started on `BoxBoxJason/sonarqube-client-go`:

1.  **Fork it!** 🍴 Head over to the `BoxBoxJason/sonarqube-client-go` GitHub page and click the 'Fork' button in the top right. This makes a copy of the project in *your* GitHub account!
2.  **Clone it!** 💻 Open your terminal and run `git clone [YOUR_FORK_URL_HERE]` (you'll get this URL from your forked repo page) to download the code to your computer.
3.  **Get on the right branch!** 🌱 Navigate into the project folder (`cd sonarqube-client-go`) and create a new branch for your work: `git checkout -b feature/analysis-endpoints`. Give it a descriptive name!
4.  **Start coding!** 🚀 This is where the magic happens! Look at how other endpoints are implemented in the project and start drafting your new analysis endpoint. Remember, small steps are key!
5.  **Commit your changes!** ✅ Once you've made some progress, save your work: `git add .` then `git commit -m "feat: implement initial analysis endpoint structure"`.
6.  **Push to your fork!** ⬆️ Send your changes up to *your* GitHub fork: `git push origin feature/analysis-endpoints`.
7.  **Open a Pull Request!** 💖 Go to your forked repository on GitHub, and you'll see an option to create a 'Pull Request' (PR) back to the original `BoxBoxJason/sonarqube-client-go` project. This is how you share your amazing work!

You're going to learn so much and make a real impact! We're here to cheer you on every step of the way. What's the very first step you'd like to try out? Or do you want a more detailed breakdown of one of these? ✨
",
  "suggestions": [
    "How do I fork a repository?",
    "Can you show me an example of `git clone`?",
    "What does 'endpoints' mean in this context?"
  ],
  "new_skills_detected": [],
  "fetch_repo_rag": "BoxBoxJason/sonarqube-client-go"
}""#;
        
        let (content, suggs, rag) = fallback_parse_json(raw);
        assert!(content.contains("YES! 🙌"));
        assert_eq!(suggs.len(), 3, "Expected 3 suggestions, found {:?}", suggs);
        assert_eq!(suggs[0], "How do I fork a repository?");
        assert_eq!(suggs[1], "Can you show me an example of `git clone`?");
        assert_eq!(suggs[2], "What does 'endpoints' mean in this context?");
        assert_eq!(rag, Some("BoxBoxJason/sonarqube-client-go".to_string()));

        // Test with spaces before colons
        let spaces_raw = r#""YES! ", "suggestions" : [ "A", "B" ], "fetch_repo_rag" : "repo/name"}"#;
        let (_, spaces_suggs, spaces_rag) = fallback_parse_json(spaces_raw);
        assert_eq!(spaces_suggs, vec!["A", "B"]);
        assert_eq!(spaces_rag, Some("repo/name".to_string()));

        // Test with truncated EOF string where "suggestions" does not exist
        let trun_raw = "{\n  \"content\": \"OMG, that's an AMAZING goal! search time: 0.000";
        let (content_trun, sugg_trun, rag_trun) = fallback_parse_json(trun_raw);
        assert_eq!(content_trun, "OMG, that's an AMAZING goal! search time: 0.000");
        assert!(sugg_trun.is_empty());
        assert!(rag_trun.is_none());
    }
}
