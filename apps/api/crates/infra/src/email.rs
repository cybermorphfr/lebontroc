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

    /// Relance : un message attend une réponse depuis plus de 24 h (F3.2).
    pub async fn send_unread_reminder(
        &self,
        to: &str,
        pseudo: &str,
        sender_pseudo: &str,
    ) -> anyhow::Result<()> {
        let subject = format!("{sender_pseudo} attend ta réponse sur Lebontroc");
        let text = format!(
            "Salut {pseudo},\n\n\
             {sender_pseudo} t'a écrit à propos d'un troc et attend ta réponse \
             depuis hier.\n\n\
             Réponds depuis tes trocs : https://lebontroc.brianplus.com/trocs\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p><strong>{sender_pseudo}</strong> t'a écrit à propos d'un troc et attend ta réponse depuis hier.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com/trocs" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Voir mes trocs</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, &subject, text, html).await
    }

    /// Prévient un proposant que sa proposition est caduque : un des objets
    /// vient d'être réservé dans un autre troc (F3.3).
    pub async fn send_proposal_invalidated(&self, to: &str, pseudo: &str) -> anyhow::Result<()> {
        let subject = "Un objet de ta proposition vient d'être réservé";
        let text = format!(
            "Salut {pseudo},\n\n\
             Un des objets de ta proposition de troc vient d'être réservé dans un \
             autre échange : ta proposition n'est plus valable.\n\n\
             Le fil regorge d'autres trouvailles — retente ta chance !\n\
             https://lebontroc.brianplus.com\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Un des objets de ta proposition de troc vient d'être réservé dans un autre échange&nbsp;: ta proposition n'est plus valable.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Explorer le fil</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// Relance J+7 : le rendez-vous de remise n'a pas encore eu lieu (F4.1).
    pub async fn send_trade_reminder(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
    ) -> anyhow::Result<()> {
        let subject = "Votre troc attend son rendez-vous";
        let text = format!(
            "Salut {pseudo},\n\n\
             Ton troc avec {other_pseudo} est accepté depuis une semaine, mais la \
             remise n'a pas encore été confirmée.\n\n\
             Convenez d'un rendez-vous dans la conversation — sans confirmation \
             sous 14 jours, le troc sera annulé et les objets libérés.\n\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Ton troc avec <strong>{other_pseudo}</strong> est accepté depuis une semaine, mais la remise n'a pas encore été confirmée.</p>
    <p>Convenez d'un rendez-vous dans la conversation — sans confirmation sous 14&nbsp;jours, le troc sera annulé et les objets libérés.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com/trocs" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Voir mes trocs</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// Annulation d'un troc jamais finalisé (J+14) : objets libérés (F4.1).
    pub async fn send_trade_auto_cancelled(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
    ) -> anyhow::Result<()> {
        let subject = "Votre troc a été annulé";
        let text = format!(
            "Salut {pseudo},\n\n\
             Le troc avec {other_pseudo} n'a pas été confirmé sous 14 jours : il \
             vient d'être annulé et vos objets sont de nouveau disponibles.\n\n\
             Rien de grave — le fil regorge d'autres trouvailles.\n\
             https://lebontroc.brianplus.com\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Le troc avec <strong>{other_pseudo}</strong> n'a pas été confirmé sous 14&nbsp;jours&nbsp;: il vient d'être annulé et vos objets sont de nouveau disponibles.</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.2 — au payeur qui n'est pas l'accepteur : la soulte est à régler.
    pub async fn send_payment_due(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        amount_cents: i32,
        delai_heures: i64,
    ) -> anyhow::Result<()> {
        let amount = amount_cents / 100;
        let subject = "Ton troc est accepté — règle la soulte pour le confirmer";
        let text = format!(
            "Salut {pseudo},\n\n\
             Bonne nouvelle : {other_pseudo} a accepté ton troc ! Il comprend une \
             soulte de {amount} € de ta part.\n\n\
             Préautorise-la sous {delai_heures} h pour confirmer le troc — l'argent \
             est simplement bloqué sur ta carte, il ne partira qu'à la remise des \
             objets. Sans règlement, le troc sera annulé.\n\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Bonne nouvelle&nbsp;: <strong>{other_pseudo}</strong> a accepté ton troc&nbsp;! Il comprend une soulte de <strong>{amount}&nbsp;€</strong> de ta part.</p>
    <p>Préautorise-la sous {delai_heures}&nbsp;h pour confirmer le troc — l'argent est simplement bloqué sur ta carte, il ne partira qu'à la remise des objets. Sans règlement, le troc sera annulé.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com/trocs" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Régler la soulte</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.2 — au bénéficiaire : la soulte est séquestrée, le troc est confirmé.
    pub async fn send_payment_escrowed(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        amount_cents: i32,
    ) -> anyhow::Result<()> {
        let amount = amount_cents / 100;
        let subject = "La soulte est sécurisée — organisez la remise";
        let text = format!(
            "Salut {pseudo},\n\n\
             Les {amount} € de soulte de ton troc avec {other_pseudo} sont \
             sécurisés : ils sont bloqués par la plateforme et te seront \
             transférés dès la remise confirmée par vos deux codes.\n\n\
             Convenez d'un rendez-vous dans la conversation.\n\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Les <strong>{amount}&nbsp;€</strong> de soulte de ton troc avec <strong>{other_pseudo}</strong> sont sécurisés&nbsp;: ils sont bloqués par la plateforme et te seront transférés dès la remise confirmée par vos deux codes.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com/trocs" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Organiser la remise</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.2 — au bénéficiaire à la remise : la soulte est transférée.
    pub async fn send_payment_released_beneficiary(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        net_cents: i32,
    ) -> anyhow::Result<()> {
        let amount = net_cents / 100;
        let subject = "Troc finalisé — la soulte t'a été transférée";
        let text = format!(
            "Salut {pseudo},\n\n\
             Le troc avec {other_pseudo} est finalisé : les {amount} € de soulte \
             t'ont été transférés. Merci d'avoir troqué plutôt qu'acheté !\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Le troc avec <strong>{other_pseudo}</strong> est finalisé&nbsp;: les <strong>{amount}&nbsp;€</strong> de soulte t'ont été transférés. Merci d'avoir troqué plutôt qu'acheté&nbsp;!</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.2 — au payeur à la remise : la préautorisation est capturée.
    pub async fn send_payment_released_payer(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        amount_cents: i32,
    ) -> anyhow::Result<()> {
        let amount = amount_cents / 100;
        let subject = "Troc finalisé — la soulte a été débitée";
        let text = format!(
            "Salut {pseudo},\n\n\
             Le troc avec {other_pseudo} est finalisé : les {amount} € de soulte \
             préautorisés ont été débités et transférés. Merci d'avoir troqué \
             plutôt qu'acheté !\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Le troc avec <strong>{other_pseudo}</strong> est finalisé&nbsp;: les <strong>{amount}&nbsp;€</strong> de soulte préautorisés ont été débités et transférés. Merci d'avoir troqué plutôt qu'acheté&nbsp;!</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.2 — au payeur d'un troc annulé : rien ne sera débité.
    pub async fn send_payment_cancelled_payer(
        &self,
        to: &str,
        pseudo: &str,
        amount_cents: i32,
    ) -> anyhow::Result<()> {
        let amount = amount_cents / 100;
        let subject = "Troc annulé — tu ne seras pas débité";
        let text = format!(
            "Salut {pseudo},\n\n\
             Le troc est annulé : la préautorisation de {amount} € a été libérée, \
             rien ne sera débité. Selon ta banque, le déblocage peut prendre \
             quelques jours.\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Le troc est annulé&nbsp;: la préautorisation de <strong>{amount}&nbsp;€</strong> a été libérée, rien ne sera débité. Selon ta banque, le déblocage peut prendre quelques jours.</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.3 — à l'autre partie quand un troc par envoi est accepté : format,
    /// relais et frais à régler sous 24 h.
    pub async fn send_shipping_setup(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
    ) -> anyhow::Result<()> {
        let subject = "Ton troc est accepté — prépare ton envoi";
        let text = format!(
            "Salut {pseudo},\n\n\
             Bonne nouvelle : le troc avec {other_pseudo} est accepté, par envoi !\n\n\
             Sous 24 h : choisis le format de ton colis, le point relais où tu \
             recevras le sien, et règle les frais d'envoi (bloqués sur ta carte, \
             débités seulement quand tout est bien arrivé). Sans règlement, le \
             troc sera annulé.\n\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Bonne nouvelle&nbsp;: le troc avec <strong>{other_pseudo}</strong> est accepté, par envoi&nbsp;!</p>
    <p>Sous 24&nbsp;h&nbsp;: choisis le format de ton colis, le point relais où tu recevras le sien, et règle les frais d'envoi (bloqués sur ta carte, débités seulement quand tout est bien arrivé). Sans règlement, le troc sera annulé.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com/trocs" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Préparer mon envoi</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.3 — au destinataire : son colis est arrivé au point relais.
    pub async fn send_parcel_arrived(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        relay_name: &str,
    ) -> anyhow::Result<()> {
        let subject = "Ton colis est arrivé au point relais";
        let text = format!(
            "Salut {pseudo},\n\n\
             Le colis de {other_pseudo} t'attend au relais « {relay_name} ».\n\n\
             Va le récupérer, puis confirme dans l'app que tout est en ordre — \
             sans nouvelle de ta part 72 h après le retrait, l'échange sera \
             considéré comme réussi.\n\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Le colis de <strong>{other_pseudo}</strong> t'attend au relais «&nbsp;{relay_name}&nbsp;».</p>
    <p>Va le récupérer, puis confirme dans l'app que tout est en ordre — sans nouvelle de ta part 72&nbsp;h après le retrait, l'échange sera considéré comme réussi.</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com/trocs" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Voir mon troc</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.3 — rappel de dépôt J+2 / J+4.
    pub async fn send_drop_reminder(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        dernier_rappel: bool,
    ) -> anyhow::Result<()> {
        let subject = if dernier_rappel {
            "Dernier rappel : dépose ton colis"
        } else {
            "Ton colis attend d'être déposé"
        };
        let urgence = if dernier_rappel {
            "Sans dépôt sous 24 h, le troc sera annulé."
        } else {
            "Dépose-le dans un point relais dès que possible."
        };
        let text = format!(
            "Salut {pseudo},\n\n\
             {other_pseudo} attend ton colis — il n'a pas encore été déposé.\n\
             {urgence}\n\n\
             Ton code de dépôt est dans l'app :\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p><strong>{other_pseudo}</strong> attend ton colis — il n'a pas encore été déposé. {urgence}</p>
    <p style="text-align:center;margin:24px 0">
      <a href="https://lebontroc.brianplus.com/trocs" style="background:#c67139;color:#f5ead8;text-decoration:none;padding:12px 28px;border-radius:999px;display:inline-block">Voir mon code de dépôt</a>
    </p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.3 — aux deux parties d'un troc envoi qui a échoué.
    pub async fn send_shipping_failed(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        gele: bool,
    ) -> anyhow::Result<()> {
        let subject = if gele {
            "Votre troc est gelé — nous regardons ce qui s'est passé"
        } else {
            "Troc annulé — les colis n'ont pas été déposés"
        };
        let corps = if gele {
            "Un colis a voyagé mais pas l'autre : le troc est gelé le temps \
             d'examiner la situation. Les préautorisations sont libérées, rien \
             ne sera débité. Nous revenons vers vous rapidement."
        } else {
            "Aucun colis n'a été déposé dans les temps : le troc est annulé, \
             les objets sont de nouveau disponibles et rien ne sera débité."
        };
        let text = format!(
            "Salut {pseudo},\n\n\
             À propos de ton troc avec {other_pseudo} : {corps}\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>À propos de ton troc avec <strong>{other_pseudo}</strong>&nbsp;: {corps}</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.3 — aux deux parties : troc envoi finalisé (souvent asynchrone).
    pub async fn send_trade_finalized_shipping(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
    ) -> anyhow::Result<()> {
        let subject = "🎉 Troc finalisé — les objets ont voyagé";
        let text = format!(
            "Salut {pseudo},\n\n\
             Les deux colis de ton troc avec {other_pseudo} sont bien arrivés : \
             le troc est finalisé. Merci d'avoir troqué plutôt qu'acheté !\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>Les deux colis de ton troc avec <strong>{other_pseudo}</strong> sont bien arrivés&nbsp;: le troc est finalisé. Merci d'avoir troqué plutôt qu'acheté&nbsp;!</p>
    <p>À très vite,<br/>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.3 — à l'admin : un troc vient d'être gelé, examen manuel requis.
    pub async fn send_admin_dispute(
        &self,
        to: &str,
        trade_id: &str,
        details: &str,
    ) -> anyhow::Result<()> {
        let subject = "⚠️ Litige gelé — examen manuel requis";
        let text = format!(
            "Un troc vient de passer en litige gelé.\n\n\
             Troc : {trade_id}\nDétails : {details}\n\n\
             Résolution manuelle en attendant F5.2 (capture ou libération des \
             paiements via SQL, voir la doc d'exploitation).\n"
        );
        let html = format!(
            r#"<div style="font-family:monospace;padding:24px">
  <p><strong>Litige gelé — examen manuel requis</strong></p>
  <p>Troc : {trade_id}<br/>Détails : {details}</p>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F5.2 — à l'autre partie : un dossier de litige vient d'être ouvert.
    pub async fn send_dispute_opened(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
        reason: &str,
    ) -> anyhow::Result<()> {
        let subject = "Un dossier a été ouvert sur ton troc — ta version compte";
        let text = format!(
            "Salut {pseudo},\n\n\
             {other_pseudo} a signalé un problème sur votre troc (motif : \
             {reason}). Le troc est suspendu le temps de l'examen.\n\n\
             Tu as 72 h pour donner ta version et joindre tes photos, depuis \
             la page du troc. Ensuite, l'équipe tranche sous 7 jours.\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             L'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p><strong>{other_pseudo}</strong> a signalé un problème sur votre troc (motif&nbsp;: {reason}). Le troc est suspendu le temps de l'examen.</p>
    <p>Tu as <strong>72&nbsp;h</strong> pour donner ta version et joindre tes photos, depuis la page du troc. Ensuite, l'équipe tranche sous 7&nbsp;jours.</p>
    <p>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F5.2 — aux deux parties : le dossier est tranché.
    pub async fn send_dispute_resolved(
        &self,
        to: &str,
        pseudo: &str,
        outcome_text: &str,
    ) -> anyhow::Result<()> {
        let subject = "Ton dossier de litige est tranché";
        let text = format!(
            "Salut {pseudo},\n\n\
             L'examen de votre troc est terminé : {outcome_text}\n\n\
             Le détail est visible sur la page du troc.\n\
             https://lebontroc.brianplus.com/trocs\n\n\
             L'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>L'examen de votre troc est terminé&nbsp;: {outcome_text}</p>
    <p>Le détail est visible sur la page du troc.</p>
    <p>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F5.2 — sanction automatique (avertissement, restriction, bannissement).
    pub async fn send_sanction(
        &self,
        to: &str,
        pseudo: &str,
        sanction_text: &str,
    ) -> anyhow::Result<()> {
        let subject = "Important — au sujet de ton compte Lebontroc";
        let text = format!(
            "Salut {pseudo},\n\n\
             {sanction_text}\n\n\
             Si tu penses qu'il y a une erreur, réponds à cet e-mail : un \
             humain te lira.\n\n\
             L'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>{sanction_text}</p>
    <p>Si tu penses qu'il y a une erreur, réponds à cet e-mail&nbsp;: un humain te lira.</p>
    <p>L'équipe Lebontroc</p>
  </div>
</div>"#
        );
        self.send(to, subject, text, html).await
    }

    /// F4.2 — aux deux parties : troc annulé faute de paiement dans les temps.
    pub async fn send_trade_payment_expired(
        &self,
        to: &str,
        pseudo: &str,
        other_pseudo: &str,
    ) -> anyhow::Result<()> {
        let subject = "Troc annulé — la soulte n'a pas été réglée à temps";
        let text = format!(
            "Salut {pseudo},\n\n\
             La soulte du troc avec {other_pseudo} n'a pas été réglée dans les \
             temps : le troc est annulé et les objets sont de nouveau \
             disponibles.\n\n\
             Rien de grave — une nouvelle proposition est toujours possible.\n\
             https://lebontroc.brianplus.com\n\n\
             À très vite,\nL'équipe Lebontroc\n"
        );
        let html = format!(
            r#"<div style="font-family:Figtree,system-ui,sans-serif;background:#f5ead8;color:#201e1d;padding:32px">
  <div style="max-width:480px;margin:0 auto;background:#ebddc5;border-radius:32px;padding:32px">
    <p style="font-size:24px;margin:0 0 16px">Lebontroc</p>
    <p>Salut {pseudo},</p>
    <p>La soulte du troc avec <strong>{other_pseudo}</strong> n'a pas été réglée dans les temps&nbsp;: le troc est annulé et les objets sont de nouveau disponibles.</p>
    <p>Rien de grave — une nouvelle proposition est toujours possible.</p>
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
