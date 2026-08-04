//! Envoi d'e-mails transactionnels — SMTP (Mailpit en dev et bêta fermée).

use std::sync::{Arc, Mutex};

use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

/// E-mail capturé par le mode test.
#[derive(Debug, Clone)]
pub struct CapturedEmail {
    pub to: String,
    pub subject: String,
    pub text: String,
}

/// Expéditeur d'e-mails. `Capture` sert aux tests d'intégration.
#[derive(Clone)]
pub enum EmailSender {
    Smtp {
        transport: Box<AsyncSmtpTransport<Tokio1Executor>>,
        from: Box<Mailbox>,
    },
    Capture(Arc<Mutex<Vec<CapturedEmail>>>),
}

impl EmailSender {
    /// Construit le transport SMTP depuis la configuration.
    /// `tls` : "none" (Mailpit), "starttls" ou "tls".
    pub fn smtp(
        host: &str,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        tls: &str,
        from: &str,
    ) -> anyhow::Result<Self> {
        let mut builder = match tls {
            "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?,
            "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(host)?,
            _ => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
        };
        builder = builder.port(port);
        if let (Some(username), Some(password)) = (username, password) {
            builder = builder.credentials(Credentials::new(username, password));
        }
        Ok(EmailSender::Smtp {
            transport: Box::new(builder.build()),
            from: Box::new(
                from.parse()
                    .map_err(|e| anyhow::anyhow!("SMTP_FROM invalide : {e}"))?,
            ),
        })
    }

    pub fn capture() -> (Self, Arc<Mutex<Vec<CapturedEmail>>>) {
        let store = Arc::new(Mutex::new(Vec::new()));
        (EmailSender::Capture(store.clone()), store)
    }

    async fn send(
        &self,
        to: &str,
        subject: &str,
        text: String,
        html: String,
    ) -> anyhow::Result<()> {
        match self {
            EmailSender::Smtp { transport, from } => {
                let message = Message::builder()
                    .from((**from).clone())
                    .to(to
                        .parse()
                        .map_err(|e| anyhow::anyhow!("destinataire invalide : {e}"))?)
                    .subject(subject)
                    .multipart(MultiPart::alternative_plain_html(text, html))?;
                transport.send(message).await?;
                Ok(())
            }
            EmailSender::Capture(store) => {
                store
                    .lock()
                    .expect("verrou des e-mails capturés")
                    .push(CapturedEmail {
                        to: to.to_string(),
                        subject: subject.to_string(),
                        text,
                    });
                Ok(())
            }
        }
    }

    /// Prévient le proposant qu'une proposition a expiré sans réponse (F3.1).
    pub async fn send_proposal_expired(
        &self,
        to: &str,
        pseudo: &str,
        recipient_pseudo: &str,
    ) -> anyhow::Result<()> {
        let subject = "Ta proposition de troc a expiré";
        let text = format!(
            "Salut {pseudo},\n\n\
             Ta proposition de troc à {recipient_pseudo} est restée sans réponse \
             pendant 7 jours : elle vient d'expirer.\n\n\
             Pas de regret — les objets sont toujours là. Tu peux refaire une \
             proposition quand tu veux, ou en tenter une autre ailleurs.\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Ta proposition de troc à <strong>{recipient_pseudo}</strong> est restée sans réponse pendant 7&nbsp;jours&nbsp;: elle vient d'expirer.</p>
    <p>Pas de regret — les objets sont toujours là. Tu peux refaire une proposition quand tu veux, ou en tenter une autre ailleurs.</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// E-mail de vérification d'adresse (copy validée produit).
    pub async fn send_verification(
        &self,
        to: &str,
        pseudo: &str,
        link: &str,
    ) -> anyhow::Result<()> {
        let subject = "Un dernier clic et c'est parti";
        let text = format!(
            "Salut {pseudo},\n\n\
             Bienvenue sur Lebontroc ! Il ne reste qu'un clic pour activer ton compte \
             et commencer à troquer.\n\n\
             Vérifier mon e-mail : {link}\n\n\
             Le lien est valable 24 heures.\n\n\
             Tu n'as pas créé de compte ? Ignore simplement cet e-mail.\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Bienvenue sur Lebontroc&nbsp;! Il ne reste qu'un clic pour activer ton compte et commencer à troquer.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="{link}" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Vérifier mon e-mail</a>
    </p>
    <p style="font-size:13px;color:#645c50">Le lien est valable 24 heures. Si le bouton ne marche pas, copie cette adresse dans ton navigateur&nbsp;: {link}</p>
    <p style="font-size:13px;color:#645c50">Tu n'as pas créé de compte&nbsp;? Ignore simplement cet e-mail.</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }
}
