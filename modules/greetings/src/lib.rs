use plugin_macro::{def_get, def_post, def_put, def_delete, declare_routes};
use serde_json::json;

// GET routes (no body)
#[def_get("/greet/hi")]
pub fn greet_hi() -> String {
    "Hi there! 👋".to_string()
}

#[def_get("/greet/bye")]
pub fn greet_bye() -> String {
    "Goodbye! 👋".to_string()
}

#[def_get("/greet/html")]
pub fn greet_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Greetings API</title>
        <style>
            body { font-family: Arial, sans-serif; margin: 40px; }
            .greeting { color: #2563eb; font-size: 24px; }
            .method { color: #059669; font-weight: bold; }
            .endpoint { background: #f3f4f6; padding: 4px 8px; border-radius: 4px; }
        </style>
    </head>
    <body>
        <h1 class="greeting">Greetings API 🎉</h1>
        <p>FastAPI-like multi-method module example:</p>
        
        <h3>GET Routes:</h3>
        <ul>
            <li><span class="method">GET</span> <code class="endpoint">/greet/hi</code> - Text greeting</li>
            <li><span class="method">GET</span> <code class="endpoint">/greet/bye</code> - Goodbye text</li>
            <li><span class="method">GET</span> <code class="endpoint">/greet/info</code> - JSON info</li>
        </ul>
        
        <h3>POST/PUT/DELETE Routes:</h3>
        <ul>
            <li><span class="method">POST</span> <code class="endpoint">/greet/user</code> - Create user greeting</li>
            <li><span class="method">PUT</span> <code class="endpoint">/greet/message</code> - Update greeting message</li>
            <li><span class="method">DELETE</span> <code class="endpoint">/greet/reset</code> - Reset/clear data</li>
        </ul>
        
        <p><em>Try using curl or Postman to test the POST/PUT/DELETE endpoints with JSON body!</em></p>
    </body>
    </html>
    "#.to_string()
}

#[def_get("/greet/info")]
pub fn greet_info() -> serde_json::Value {
    json!({
        "module": "greetings",
        "version": "2.0.0",
        "routes": {
            "get": ["/greet/hi", "/greet/bye", "/greet/html", "/greet/info"],
            "post": ["/greet/user"],
            "put": ["/greet/message"],
            "delete": ["/greet/reset"]
        },
        "description": "A FastAPI-like multi-method greeting module"
    })
}

// POST route (with body)
#[def_post("/greet/user")]
pub fn create_user(body: &str) -> serde_json::Value {
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
    
    match parsed {
        Ok(data) => {
            let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("Anonymous");
            let age = data.get("age").and_then(|v| v.as_u64()).unwrap_or(0);
            
            json!({
                "status": "success",
                "message": format!("Hello {}! Welcome to our API.", name),
                "user": {
                    "name": name,
                    "age": age,
                    "id": format!("user_{}", chrono::Utc::now().timestamp()),
                    "greeting": format!("Hi {}, you are {} years old!", name, age)
                },
                "created_at": chrono::Utc::now().to_rfc3339()
            })
        },
        Err(_) => {
            json!({
                "status": "error",
                "message": "Invalid JSON body. Expected: {\"name\": \"string\", \"age\": number}",
                "example": {
                    "name": "John",
                    "age": 25
                }
            })
        }
    }
}

// PUT route (with body)
#[def_put("/greet/message")]
pub fn update_message(body: &str) -> serde_json::Value {
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
    
    match parsed {
        Ok(data) => {
            let message = data.get("message").and_then(|v| v.as_str()).unwrap_or("Hello World");
            let language = data.get("language").and_then(|v| v.as_str()).unwrap_or("en");
            
            let localized_message = match language {
                "vi" => format!("Xin chào! {}", message),
                "es" => format!("¡Hola! {}", message),
                "fr" => format!("Bonjour! {}", message),
                "de" => format!("Hallo! {}", message),
                _ => format!("Hello! {}", message)
            };
            
            json!({
                "status": "updated",
                "original_message": message,
                "localized_message": localized_message,
                "language": language,
                "updated_at": chrono::Utc::now().to_rfc3339()
            })
        },
        Err(_) => {
            json!({
                "status": "error",
                "message": "Invalid JSON body. Expected: {\"message\": \"string\", \"language\": \"string\"}",
                "supported_languages": ["en", "vi", "es", "fr", "de"],
                "example": {
                    "message": "How are you?",
                    "language": "vi"
                }
            })
        }
    }
}

// DELETE route (with body)
#[def_delete("/greet/reset")]
pub fn reset_data(body: &str) -> serde_json::Value {
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
    
    match parsed {
        Ok(data) => {
            let confirm = data.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);
            let target = data.get("target").and_then(|v| v.as_str()).unwrap_or("all");
            
            if confirm {
                json!({
                    "status": "deleted",
                    "message": format!("Successfully reset {} data", target),
                    "target": target,
                    "deleted_at": chrono::Utc::now().to_rfc3339(),
                    "items_deleted": match target {
                        "users" => 42,
                        "messages" => 128,
                        "all" => 170,
                        _ => 0
                    }
                })
            } else {
                json!({
                    "status": "cancelled",
                    "message": "Reset cancelled. Set 'confirm': true to proceed.",
                    "warning": "This action cannot be undone!"
                })
            }
        },
        Err(_) => {
            json!({
                "status": "error",
                "message": "Invalid JSON body. Expected: {\"confirm\": boolean, \"target\": \"string\"}",
                "example": {
                    "confirm": true,
                    "target": "users"
                },
                "valid_targets": ["users", "messages", "all"]
            })
        }
    }
}

// Khai báo registry và export manifest tự động (không cần liệt kê routes)
declare_routes!();