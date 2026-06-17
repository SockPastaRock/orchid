use crate::types::TokenBudget;

/// Resolve a persona's system prompt by loading and concatenating prompt files.
pub fn resolve_system_prompt(persona_name: &str, config: &crate::Config) -> Result<String, String> {
    let personas = match config.extra.get("personas") {
        Some(v) => v,
        None => return Err("no personas defined in config".to_string()),
    };

    let persona = match personas.get(persona_name) {
        Some(v) => v,
        None => return Err(format!("persona '{}' not found in config", persona_name)),
    };

    let prompts = persona
        .get("prompts")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("persona '{}' has no prompts array", persona_name))?;

    let prompts_dir = crate::get_orchid_dir()?.join("system-prompts");

    let mut parts = Vec::new();
    for p in prompts {
        let name = p
            .as_str()
            .ok_or_else(|| "prompt name must be a string".to_string())?;
        let path = prompts_dir.join(format!("{}.md", name));
        let content = std::fs::read_to_string(&path)
            .map_err(|_| format!("system prompt file not found: {}", path.display()))?;
        parts.push(content.trim().to_string());
    }

    Ok(parts.join("\n\n"))
}

/// Read persona-level limits from config and merge over the global limits.
/// Persona limits shadow global ones only for fields that are explicitly set.
pub fn resolve_persona_budget(
    persona_name: &str,
    global: &TokenBudget,
    config: &crate::Config,
) -> TokenBudget {
    let persona_limits = config
        .extra
        .get("personas")
        .and_then(|p| p.get(persona_name))
        .and_then(|p| p.get("limits"));

    let Some(limits) = persona_limits else {
        return global.clone();
    };

    let warn_threshold = limits
        .get("token_warn_threshold")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(global.warn_threshold);

    let hard_limit = limits
        .get("token_hard_limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(global.hard_limit);

    TokenBudget {
        warn_threshold,
        hard_limit,
    }
}
