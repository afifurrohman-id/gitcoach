use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use shared::{ChatRequest, ChatResponse, Message, Persona, DomainGoal, Role};

#[component]
fn App() -> impl IntoView {
    // Load persistent state from LocalStorage
    let initial_messages = LocalStorage::get("gitcoach_messages").unwrap_or_else(|_| Vec::new());
    let initial_skills = LocalStorage::get("gitcoach_skills").unwrap_or_else(|_| Vec::new());
    let initial_suggestions = LocalStorage::get("gitcoach_suggestions").unwrap_or_else(|_| Vec::new());
    let initial_persona: Persona = LocalStorage::get("gitcoach_persona").unwrap_or_else(|_| Persona::default());
    let initial_domain: DomainGoal = LocalStorage::get("gitcoach_domain").unwrap_or_else(|_| DomainGoal::default());

    // Reactive state for chat history and skills
    let (messages, set_messages) = signal(initial_messages);
    let (skill_tree, set_skill_tree) = signal(initial_skills);
    let (suggestions, set_suggestions) = signal(initial_suggestions);

    // Sync state to LocalStorage automatically when it changes
    Effect::new(move |_| {
        let _ = LocalStorage::set("gitcoach_messages", &messages.get());
    });
    Effect::new(move |_| {
        let _ = LocalStorage::set("gitcoach_skills", &skill_tree.get());
    });
    Effect::new(move |_| {
        let _ = LocalStorage::set("gitcoach_suggestions", &suggestions.get());
    });
    
    // Reactive state for user input
    let (input, set_input) = signal(String::new());
    
    // Reactive state for loading/typing
    let (is_loading, set_is_loading) = signal(false);

    let (persona, set_persona) = signal(initial_persona);
    let (domain, set_domain) = signal(initial_domain);

    Effect::new(move |_| {
        let _ = LocalStorage::set("gitcoach_persona", &persona.get());
    });
    Effect::new(move |_| {
        let _ = LocalStorage::set("gitcoach_domain", &domain.get());
    });

    let submit_message = Action::new_local(move |_: &()| async move {
        let current_input = input.get();
        if current_input.trim().is_empty() || is_loading.get() {
            return;
        }

        let user_msg = Message {
            role: Role::User,
            content: current_input.clone(),
        };
        
        set_messages.update(|msgs| msgs.push(user_msg.clone()));
        set_input.set(String::new());
        set_is_loading.set(true);

        let current_messages = messages.get();
        let current_persona = persona.get();
        let current_domain = domain.get();
        let current_skills = skill_tree.get();

        let request_payload = ChatRequest {
            messages: current_messages,
            persona: current_persona,
            domain: current_domain,
            skill_tree: current_skills,
        };

        let response = Request::post("http://localhost:3000/api/chat")
            .json(&request_payload)
            .expect("Failed to serialize request")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if let Ok(chat_resp) = resp.json::<ChatResponse>().await {
                    if let Some(err) = chat_resp.error {
                        set_messages.update(|msgs| msgs.push(Message {
                            role: Role::Assistant,
                            content: format!("Error: {}", err),
                        }));
                    } else {
                        // Handle new skills if detected
                        if let Some(new_skills) = chat_resp.new_skills {
                            set_skill_tree.update(|st| {
                                for skill in new_skills {
                                    if !st.contains(&skill) {
                                        st.push(skill);
                                    }
                                }
                            });
                        }
                        
                        // Set new suggestions
                        set_suggestions.set(chat_resp.suggestions);

                        set_messages.update(|msgs| msgs.push(Message {
                            role: Role::Assistant,
                            content: chat_resp.content,
                        }));
                    }
                }
            }
            Err(_) => {
                set_messages.update(|msgs| msgs.push(Message {
                    role: Role::Assistant,
                    content: "Error: Could not connect to the backend server.".to_string(),
                }));
            }
        }
        
        set_is_loading.set(false);
    });

    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        submit_message.dispatch(());
    };

    view! {
        <div class="app-container">
            <header class="header">
                <h1>"GitCoach"</h1>
            </header>

            <div class="settings-bar">
                <select 
                    class="setting-select"
                    prop:value=move || persona.get().to_string()
                    on:change=move |ev| {
                        let val = event_target_value(&ev);
                        if let Ok(p) = val.parse::<Persona>() {
                            set_persona.set(p);
                        }
                    }
                >
                    <option value="Cheerleader">"Coach Cheerleader"</option>
                    <option value="TechLead">"Strict Tech Lead"</option>
                </select>
                
                <select 
                    class="setting-select"
                    prop:value=move || domain.get().to_string()
                    on:change=move |ev| {
                        let val = event_target_value(&ev);
                        if let Ok(d) = val.parse::<DomainGoal>() {
                            set_domain.set(d);
                        }
                    }
                >
                    <option value="FrontendWeb">"Frontend Web"</option>
                    <option value="BackendAPIs">"Backend APIs"</option>
                    <option value="SystemProgramming">"System Programming"</option>
                    <option value="MachineLearning">"Machine Learning"</option>
                </select>
            </div>

            <div class="chat-container" id="chat-container">
                {move || {
                    if messages.get().is_empty() {
                        view! {
                            <div class="message-wrapper assistant">
                                <div class="message assistant">
                                    "Hello! I am GitCoach. How can I assist your open-source journey today?"
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <For
                                each=move || messages.get()
                                key=|msg| msg.content.clone() // In real app, use UUID
                                children=|msg| {
                                    let is_user = msg.role == Role::User;
                                    let wrapper_class = if is_user { "message-wrapper user" } else { "message-wrapper assistant" };
                                    
                                    let is_error = msg.content.starts_with("Error:") 
                                        || msg.content.starts_with("Woops!") 
                                        || msg.content.starts_with("The Gemini API is currently experiencing")
                                        || msg.content.starts_with("Failed to communicate");

                                    let msg_class = if is_user { 
                                        "message user" 
                                    } else if is_error {
                                        "message assistant error"
                                    } else { 
                                        "message assistant" 
                                    };
                                    
                                    if is_user {
                                        view! {
                                            <div class={wrapper_class}>
                                                <div class={msg_class}>
                                                    {msg.content}
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        let safe_html = parse_markdown_to_safe_html(&msg.content);
                                        
                                        view! {
                                            <div class={wrapper_class}>
                                                <div class={msg_class} inner_html={safe_html}>
                                                </div>
                                            </div>
                                        }.into_any()
                                    }
                                }
                            />
                        }.into_any()
                    }
                }}

                {move || {
                    if is_loading.get() {
                        view! {
                            <div class="message-wrapper assistant">
                                <div class="typing-indicator">
                                    <span></span>
                                    <span></span>
                                    <span></span>
                                </div>
                            </div>
                        }.into_any()
                    } else if !suggestions.get().is_empty() {
                        view! {
                            <div class="suggestions-container">
                                <For
                                    each=move || suggestions.get()
                                    key=|s| s.clone()
                                    children=move |s| {
                                        let suggestion_text = s.clone();
                                        view! {
                                            <button 
                                                class="suggestion-chip"
                                                on:click=move |_| {
                                                    set_input.set(suggestion_text.clone());
                                                }
                                            >
                                                {s}
                                            </button>
                                        }
                                    }
                                />
                            </div>
                        }.into_any()
                    } else {
                        view! { <span style="display: none"></span> }.into_any()
                    }
                }}
            </div>

            <div class="input-area">
                <form class="input-form" on:submit=handle_submit>
                    <textarea
                        class="chat-input"
                        placeholder="Type your message... (Shift+Enter for newline)"
                        prop:value=move || input.get()
                        on:input=move |ev| set_input.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" && !ev.shift_key() {
                                ev.prevent_default();
                                submit_message.dispatch(());
                            }
                        }
                        prop:disabled=move || is_loading.get()
                        rows="2"
                    ></textarea>
                    <button type="submit" prop:disabled=move || is_loading.get() || input.get().trim().is_empty()>
                        "Send"
                    </button>
                </form>
            </div>
        </div>
    }
}

pub fn fix_bracketed_urls(text: &str) -> String {
    let mut output = String::with_capacity(text.len() + 50);
    let mut remaining = text;

    while let Some(start_idx) = remaining.find("[http") {
        output.push_str(&remaining[..start_idx]);
        
        let after_bracket = &remaining[start_idx..];
        if let Some(end_idx) = after_bracket.find(']') {
            let url = &after_bracket[1..end_idx]; 
            
            // If it's already a valid markdown link, the next char is '('
            let is_already_markdown = after_bracket[end_idx + 1..].starts_with('(');
            
            if !is_already_markdown && !url.contains(|c: char| c.is_whitespace()) {
                output.push_str(&format!("[{}]({})", url, url));
                remaining = &after_bracket[end_idx + 1..];
                continue;
            }
        }
        
        output.push_str("[http");
        remaining = &after_bracket[5..];
    }
    
    output.push_str(remaining);
    output
}

pub fn parse_markdown_to_safe_html(content: &str) -> String {
    let preprocessed_content = fix_bracketed_urls(content);
    let parser = pulldown_cmark::Parser::new(&preprocessed_content);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    
    // Ensure all links open in a new tab by replacing the anchor tag
    html_output.replace("<a href=", "<a target=\"_blank\" rel=\"noopener noreferrer\" href=")
}

pub fn is_error_message(content: &str) -> bool {
    content.starts_with("Error:") 
        || content.starts_with("Woops!") 
        || content.starts_with("The Gemini API is currently experiencing")
        || content.starts_with("Failed to communicate")
}

fn main() {
    // Initialize standard logging
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    // Mount Leptos App
    leptos::mount::mount_to_body(|| view! { <App/> })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{Message, Role};

    #[test]
    fn test_message_struct_creation() {
        // A simple test to verify our data models can be used in the frontend without issue
        let msg = Message {
            role: Role::User,
            content: "Test message from the frontend logic".to_string(),
        };
        
        assert_eq!(msg.content, "Test message from the frontend logic");
        assert_eq!(msg.role, Role::User);
    }

    #[test]
    fn test_markdown_link_target_blank() {
        let markdown = "Here is a [Google](https://google.com) link.";
        let html = parse_markdown_to_safe_html(markdown);
        
        // It should contain the expected HTML
        assert!(html.contains("<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"https://google.com\">Google</a>"));
    }

    #[test]
    fn test_markdown_newlines_and_lists() {
        let markdown = "Line 1\nLine 2\n\n- Item 1\n- Item 2";
        let html = parse_markdown_to_safe_html(markdown);
        
        assert!(html.contains("<li>Item 1</li>"));
        assert!(html.contains("<li>Item 2</li>"));
    }

    #[test]
    fn test_settings_parsing() {
        use shared::{Persona, DomainGoal};
        use std::str::FromStr;

        assert_eq!(Persona::from_str("TechLead").unwrap(), Persona::TechLead);
        assert_eq!(Persona::from_str("Cheerleader").unwrap(), Persona::Cheerleader);
        
        assert_eq!(DomainGoal::from_str("SystemProgramming").unwrap(), DomainGoal::SystemProgramming);
        assert_eq!(DomainGoal::from_str("FrontendWeb").unwrap(), DomainGoal::FrontendWeb);
        
        assert!(Persona::from_str("InvalidPersona").is_err());
        assert!(DomainGoal::from_str("InvalidDomain").is_err());
    }

    #[test]
    fn test_is_error_message() {
        assert!(is_error_message("Error: Could not connect to the backend server."));
        assert!(is_error_message("Woops! It looks like we've hit the free tier quota limit"));
        assert!(is_error_message("The Gemini API is currently experiencing extremely high demand."));
        assert!(is_error_message("Failed to communicate with AI: 500 Internal Server Error"));
        assert!(!is_error_message("Hello! I am GitCoach."));
        assert!(!is_error_message("Here is your code: Woops something else"));
    }

    #[test]
    fn test_fix_bracketed_urls() {
        // Raw bracketed URL
        let raw = "Check this out: [https://github.com/BoxBoxJason/sonarqube-client-go/issues/174]";
        let fixed = fix_bracketed_urls(raw);
        assert_eq!(fixed, "Check this out: [https://github.com/BoxBoxJason/sonarqube-client-go/issues/174](https://github.com/BoxBoxJason/sonarqube-client-go/issues/174)");

        // Already correct markdown link
        let md = "Check this out: [link](https://github.com)";
        let fixed_md = fix_bracketed_urls(md);
        assert_eq!(fixed_md, md);

        // Already correct markdown link where the text is also the URL
        let md2 = "Check this out: [https://google.com](https://google.com)";
        let fixed_md2 = fix_bracketed_urls(md2);
        assert_eq!(fixed_md2, md2);

        // Not a URL (has spaces)
        let bracketed_text = "This is [http not a url]";
        let fixed_text = fix_bracketed_urls(bracketed_text);
        assert_eq!(fixed_text, bracketed_text);
    }
}
