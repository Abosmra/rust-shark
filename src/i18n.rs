//! Lightweight EN / PT-BR localization.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    En,
    Pt,
}

impl Default for Lang {
    fn default() -> Self {
        system_default()
    }
}

/// Detect Windows UI language: Portuguese (any region, primary ID 0x16) -> Pt, else En.
pub fn system_default() -> Lang {
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    if (id & 0x03FF) == 0x16 {
        Lang::Pt
    } else {
        Lang::En
    }
}

/// Translate an English UI string into `lang`. Unknown strings fall back to `en`.
pub fn t(lang: Lang, en: &'static str) -> &'static str {
    if lang == Lang::En {
        return en;
    }
    match en {
        // Navigation & top bar
        "Home" => "Início",
        "Performance" => "Desempenho",
        "Buttons" => "Botões",
        "Macros" => "Macros",
        "Lighting" => "Iluminação",
        "Power" => "Energia",
        "Profiles" => "Perfis",
        "Settings" => "Configurações",
        "Profile" => "Perfil",
        "Not connected" => "Desconectado",
        "Saving to mouse…" => "Salvando no mouse…",
        "Connected · USB" => "Conectado · USB",
        "Connected · 2.4G" => "Conectado · 2.4G",
        "Connected over USB" => "Conectado via USB",
        "Connected over 2.4G" => "Conectado via 2.4G",
        "Changes save to the mouse automatically" => "Alterações salvas automaticamente no mouse",
        "Auto-save is off: click Save to apply" => "Auto-salvar desligado: clique em Salvar",
        "Open Rust Shark" => "Abrir Rust Shark",
        "Quit" => "Sair",

        // Banners & toasts
        "Unsaved changes" => "Alterações não salvas",
        "They aren't on your mouse yet" => "Elas ainda não foram enviadas para o mouse",
        "Save to mouse" => "Salvar no mouse",
        "Discard" => "Descartar",
        "Go back to what's on the mouse" => "Voltar ao que está salvo no mouse",
        "Saved to the mouse" => "Salvo no mouse",
        "Changes discarded" => "Alterações descartadas",
        "The mouse missed a change: re-sending your settings" => "O mouse perdeu uma alteração: reenviando configurações",
        "The mouse isn't confirming changes. They were sent, but may not have applied." => "O mouse não confirmou as alterações. Elas foram enviadas, mas podem não ter sido aplicadas.",
        "Imported your settings from the official app" => "Configurações importadas do app oficial",
        "Your settings file couldn't be read; kept a copy as config.json.bad" => "Não foi possível ler o arquivo de configuração; uma cópia foi salva como config.json.bad",
        "Imported your macro library from the official app" => "Biblioteca de macros importada do app oficial",
        "Low battery (≤20%) — connect your mouse to charge" => "Bateria baixa (≤20%) — conecte o mouse para carregar",

        // Home
        "Overview" => "Visão geral",
        "Waiting for the mouse…" => "Aguardando o mouse…",
        "Move the mouse to wake it" => "Mova o mouse para acordá-lo",
        "Reported in 10% steps" => "Informado em passos de 10%",
        "Manage" => "Gerenciar",
        "Open ›" => "Abrir ›",
        "DPI" => "DPI",
        "Polling rate" => "Taxa de polling",
        "Sensor" => "Sensor",
        "Motion sync" => "Motion sync",
        "Angle snap" => "Angle snap",
        "Ripple control" => "Controle de ripple",
        "Key response" => "Tempo de resposta",
        "Lighting & power" => "Iluminação e energia",
        "Effect" => "Efeito",
        "Brightness" => "Brilho",
        "Speed" => "Velocidade",
        "Colors" => "Cores",
        "Per DPI level" => "Por nível de DPI",
        "Sleep after" => "Suspender após",
        "Deep sleep" => "Suspensão profunda",
        "On" => "Ligado",
        "Off" => "Desligado",

        // Battery states
        "On battery" => "Na bateria",
        "Fully charged" => "Carga completa",
        "Charging" => "Carregando",
        "Unknown" => "Desconhecido",

        // Power
        "Battery" => "Bateria",
        "Sleep" => "Suspensão",
        "Dims and saves power when idle" => "Escurece e economiza energia quando ocioso",
        "Deep sleep after" => "Suspensão profunda após",
        "Fully powers down; click to wake" => "Desliga totalmente; clique para acordar",
        "Low battery warning" => "Alerta de bateria baixa",
        "Show a warning when battery drops to 20% or below" => "Mostra um aviso quando a bateria atingir 20% ou menos",

        // Profiles
        "Saved profiles" => "Perfis salvos",
        "ACTIVE ON MOUSE" => "ATIVO NO MOUSE",
        "click to apply" => "clique para aplicar",
        "+  New" => "+  Novo",
        "Duplicate" => "Duplicar",
        "Rename" => "Renomear",
        "Delete" => "Excluir",
        "Save" => "Salvar",
        "Tools" => "Ferramentas",
        "Re-send everything" => "Reenviar tudo",
        "Writes the whole active profile to the mouse" => "Grava todo o perfil ativo no mouse",
        "Sync now" => "Sincronizar agora",
        "Sending full profile…" => "Enviando perfil completo…",
        "Export / Import profile" => "Exportar / Importar perfil",
        "Save the active profile as JSON or load one from disk" => "Salva o perfil ativo em JSON ou carrega um do disco",
        "Export…" => "Exportar…",
        "Import…" => "Importar…",
        "Profile exported" => "Perfil exportado",
        "Couldn't save the file" => "Não foi possível salvar o arquivo",
        "That file isn't a valid Rust Shark profile" => "Esse arquivo não é um perfil válido do Rust Shark",
        "Import from official app" => "Importar do app oficial",
        "Adds the profiles saved by the Attack Shark software" => "Adiciona os perfis salvos pelo software oficial da Attack Shark",
        "Import" => "Importar",
        "No official app data found" => "Nenhum dado do app oficial encontrado",
        "Factory defaults" => "Padrão de fábrica",
        "Resets this profile to the mouse's defaults" => "Restaura este perfil para os padrões de fábrica do mouse",
        "Reset" => "Restaurar",

        // Settings & Backups
        "Your mouse" => "Seu mouse",
        "Mouse color" => "Cor do mouse",
        "Which X6 the pictures show" => "Qual modelo X6 aparece nas imagens",
        "Black" => "Preto",
        "White" => "Branco",
        "Language" => "Idioma",
        "Interface language" => "Idioma da interface",
        "Saving" => "Salvamento",
        "Auto-save to mouse" => "Salvar automaticamente no mouse",
        "On: every change goes to the mouse right away. Off: changes wait until you press Save Changes." => "Ligado: cada alteração vai para o mouse na hora. Desligado: aguarda você clicar em Salvar.",
        "Auto-save on" => "Auto-salvar ligado",
        "Auto-save off: use Save Changes to apply" => "Auto-salvar desligado: clique em Salvar para aplicar",
        "Windows" => "Windows",
        "Start with Windows" => "Iniciar com o Windows",
        "Open Rust Shark when you sign in" => "Abre o Rust Shark ao entrar no Windows",
        "Windows didn't allow changing the startup setting" => "O Windows não permitiu alterar a inicialização",
        "Start minimized" => "Iniciar minimizado",
        "When it starts with Windows, stay in the system tray instead of opening the window" => "Ao iniciar com o Windows, permanece na bandeja em vez de abrir a janela",
        "Close to system tray" => "Fechar para a bandeja",
        "Closing the window keeps Rust Shark running by the clock. Quit from the tray icon's menu." => "Fechar a janela mantém o Rust Shark ativo perto do relógio. Use Sair no menu da bandeja.",
        "Backups" => "Backups",
        "Save a backup" => "Salvar um backup",
        "All profiles, macros and settings in one file" => "Todos os perfis, macros e configurações em um arquivo",
        "Save…" => "Salvar…",
        "Backup saved" => "Backup salvo",
        "Restore a backup" => "Restaurar um backup",
        "Replaces everything here with a backup file" => "Substitui tudo aqui por um arquivo de backup",
        "Open…" => "Abrir…",
        "That file isn't a Rust Shark backup" => "Esse arquivo não é um backup do Rust Shark",
        "Automatic backups" => "Backups automáticos",
        "Open folder" => "Abrir pasta",
        "Restore" => "Restaurar",
        "That backup can't be read" => "Não foi possível ler esse backup",
        "Restore this backup?" => "Restaurar este backup?",
        "Backup restored" => "Backup restaurado",
        "Updates" => "Atualizações",
        "Check for updates" => "Verificar atualizações",
        "At startup, ask GitHub if a newer release exists. Nothing downloads on its own." => "Na inicialização, consulta o GitHub por uma nova versão. Nada é baixado automaticamente.",
        "About" => "Sobre",
        "Control app for the Attack Shark X6" => "Aplicativo de controle para o Attack Shark X6",
        "Input icons" => "Ícones de entrada",
        "Keyboard & mouse icons by amanz. Thank you!" => "Ícones de teclado e mouse por amanz. Obrigado!",
        "Settings icon" => "Ícone de configurações",
        "Animated gear by Icons8" => "Engrenagem animada por Icons8",
        "Settings folder" => "Pasta de configurações",
        "Open" => "Abrir",
        "Cancel" => "Cancelar",

        _ => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_returns_input_unchanged() {
        assert_eq!(t(Lang::En, "Settings"), "Settings");
        assert_eq!(t(Lang::En, "Unmapped string"), "Unmapped string");
    }

    #[test]
    fn portuguese_translates_key_strings() {
        assert_eq!(t(Lang::Pt, "Settings"), "Configurações");
        assert_eq!(t(Lang::Pt, "Home"), "Início");
        assert_eq!(t(Lang::Pt, "Quit"), "Sair");
        assert_eq!(t(Lang::Pt, "Low battery warning"), "Alerta de bateria baixa");
        assert_eq!(t(Lang::Pt, "Unmapped string"), "Unmapped string");
    }
}
