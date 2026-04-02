use crate::HashMap;

const ENGLISH_STATUS_MESSAGES: &[&str] = &[
    "License is valid and unlocked.",
    "License not found.",
    "Machine limit reached. You can regenerate your license code to remove old machines from your license.",
    "Your trial has ended. Please purchase a license to continue using all the features.",
    "Your license is inactive. Please renew your license to continue using all the features.",
    "The offline code was incorrect.",
    "Offline codes are disabled for this software.",
    "The license code is invalid.",
    "This machine has been deactivated. Please enter another license code.",
    "Unknown error. You may need to update your software. Please contact support with the error code if that does not fix the issue.",
];

const FRENCH_STATUS_MESSAGES: &[&str] = &[
    "La licence est valide et déverrouillée.",
    "Licence introuvable.",
    "Limite de machines atteinte. Vous pouvez régénérer votre code de licence pour supprimer les anciennes machines de votre licence.",
    "Votre période d’essai est terminée. Veuillez acheter une licence pour continuer à utiliser toutes les fonctionnalités.",
    "Votre licence est inactive. Veuillez renouveler votre licence pour continuer à utiliser toutes les fonctionnalités.",
    "Le code hors ligne est incorrect.",
    "Les codes hors ligne sont désactivés pour ce logiciel.",
    "Le code de licence est invalide.",
    "Cette machine a été désactivée. Veuillez saisir un autre code de licence.",
    "Erreur inconnue. Vous devrez peut-être mettre à jour votre logiciel. Veuillez contacter le support avec le code d’erreur si cela ne résout pas le problème.",
];

const SPANISH_STATUS_MESSAGES: &[&str] = &[
    "La licencia es válida y está desbloqueada.",
    "Licencia no encontrada.",
    "Se alcanzó el límite de dispositivos. Puede regenerar su código de licencia para eliminar los dispositivos antiguos de su licencia.",
    "Su período de prueba ha finalizado. Por favor, compre una licencia para seguir usando todas las funciones.",
    "Su licencia está inactiva. Por favor, renueve su licencia para seguir usando todas las funciones.",
    "El código sin conexión es incorrecto.",
    "Los códigos sin conexión están deshabilitados para este software.",
    "El código de licencia no es válido.",
    "Este dispositivo ha sido desactivado. Por favor, introduzca otro código de licencia.",
    "Error desconocido. Es posible que necesite actualizar su software. Por favor, contacte con el soporte e incluya el código de error si esto no resuelve el problema.",
];

const SUPPORTED_LANGUAGES: &[&str] = &[
    "en",
    "fr",
    "es",
];

/// Normalizes a locale string.
fn normalize_locale(s: &str) -> String {
    let mut s = s.trim().to_lowercase();

    // Remove encoding (e.g. .UTF-8)
    if let Some(idx) = s.find('.') {
        s.truncate(idx);
    }

    // Remove modifiers (e.g. @euro)
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }

    // Normalize separator
    s = s.replace('_', "-");

    s
}
/// Extracts the base language from a locale string (e.g. "en" from "en-US").
fn base_language(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}
/// Gets the status message corresponding to a status code and language.
pub(crate) fn get_status_message_from_code(code: i32) -> String {
    let language = super::stats::language();
    let display_language_normalized = normalize_locale(&language.display_language);
    let users_language_normalized = normalize_locale(&language.users_language);
    let base_language_display_lang = base_language(&display_language_normalized);
    let base_language_users_lang = base_language(&users_language_normalized);
    let language = if language.display_language.len() != 0 && SUPPORTED_LANGUAGES.contains(&base_language_display_lang) {
        &base_language_display_lang
    } else if language.users_language.len() != 0 && SUPPORTED_LANGUAGES.contains(&base_language_users_lang) {
        &base_language_users_lang
    } else {
        "en"
    };
    let messages = match language {
        "en" => ENGLISH_STATUS_MESSAGES,
        "fr" => FRENCH_STATUS_MESSAGES,
        "es" => SPANISH_STATUS_MESSAGES,
        _ => ENGLISH_STATUS_MESSAGES,
    };

    for m in 0..8 {
        if code & (1 << m) != 0 {
            if let Some(msg) = messages.get(m) {
                return msg.to_string();
            }
        }
    }
    messages.last().unwrap_or(&"Unknown status code. Please contact support.").to_string()
}