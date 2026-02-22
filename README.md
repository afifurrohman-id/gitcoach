# GitCoach: The AI Open-Source Mentor
> Witness the Agentic RAG, Dynamic Suggestions, and Skill Tree Memory in action natively in the browser!

![GitCoach E2E Demo](https://github.com/user-attachments/assets/6f63c759-6df4-4e54-848a-76fc537c4df3)

---

## 🎯 What is this project about?
GitCoach is a cutting-edge, "Local-First" AI Mentor built entirely in Rust. It serves as an interactive bridge between junior developers and the massive ecosystem of Open-Source Software. By uniquely pairing the Google Gemini API with the GitHub API, GitCoach dynamically mentors users, finds accessible issues for them to work on, and directly guides them through the contribution process.

## 🚀 What's the Goal?
To eliminate "Imposter Syndrome" and the high barrier to entry in open-source software. Most beginners want to contribute but don't know *where* to start or *how* to set up a project. GitCoach aims to completely hand-hold developers through their first Pull Request.

## 💥 The Problem it Solves
"Analysis Paralysis." When a developer looks at a massive codebase, they freeze. GitHub has a "Good First Issue" label, but even those can be daunting. GitCoach solves this by serving as a pair-programmer that actually understands your current skill level, reads the repository's documentation for you, and breaks the goal down into microscopic, achievable steps based on your specific Domain goal (e.g., Frontend Web vs System Programming). 

---

## ✅ Features

GitCoach isn't just a generic "chatbot wrapper." It utilizes advanced Agentic AI workflows to create an incredibly personalized and defensible experience:

### 1. Agentic RAG Loop (The Core) 🤖
If GitCoach recommends an issue in the `backendsystems/nibble` repository, and the user asks *"How do I start?"*, the AI doesn't hallucinate an answer. 
Instead, it emits a strict JSON tool call: `"fetch_repo_rag": "backendsystems/nibble"`. 
The Rust backend intercepts this, recursively pauses the conversation, calls the GitHub API to fetch the *actual* `CONTRIBUTING.md` of that specific repository, and injects it back into Gemini's context window. The AI then spits out the *exact* terminal `git clone` commands the user needs to run. **It reads the documentation for you.**

### 2. The Persistent "Skill Tree" (Advanced Contextual Memory) 🧠
The AI is instructed to hunt for "ah-ha!" moments. If the user demonstrates they finally understand how "Rust Lifetimes" work, the AI outputs `"new_skills_detected": ["Rust Lifetimes"]`. 
The Leptos frontend parses this JSON, saves it to the browser's persistent `LocalStorage`, and attaches that Skill Tree to every future API request. The AI learns what you know, and stops over-explaining basic concepts as you grow!

### 3. Dynamic Follow-Up Suggestions 💡
At the end of every message, Gemini outputs a JSON array of `suggestions`. The frontend beautifully maps these to clickable UI chips directly below the chat bubble. If the user doesn't know what to type, they can just click a chip to organically drive the conversation forward.

### 4. Stateless "Local-First" Privacy 🔒
GitCoach completely avoids exposing a massive, expensive PostgreSQL database. All Personal Identity, Chat History, and Skill Tree progression are saved purely in the user's browser via WebAssembly `gloo-storage`. You own your prompt data.

---


## 🛠️ How to Run

This project utilizes a modern Rust Monorepo structure containing `backend`, `frontend`, and `shared` logic.

### Prerequisites
1. Install [Rust](https://rustup.rs/) (cargo)
2. Install [Trunk](https://trunkrs.dev/) (for Leptos WebAssembly compilation):
   ```bash
   cargo install trunk
   ```
3. Add WebAssembly target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```
4. [Get a Google Gemini API Key from Google AI Studio](https://ai.google.dev/gemini-api/docs/api-key).

### Quick Start

1. **Clone the repo**
2. **Setup your environment variables:**
   Create a `.env` file in the root of the `backend` folder:
   ```bash
   cd backend
   echo "GEMINI_API_KEY=your_actual_api_key_here" > .env
   ```

3. **Start the Axum Backend (Terminal 1)**
   ```bash
   cd backend
   cargo run
   ```
   *The server runs on http://localhost:3000.*

4. **Start the Leptos Frontend (Terminal 2)**
   ```bash
   cd frontend
   trunk serve --open
   ```
   *The Web UI will automatically compile and open in your browser at http://localhost:8080.*

---
*Built tightly with Axum, Leptos, and Gemini.*
