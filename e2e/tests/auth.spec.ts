import { expect, test, type Page } from "@playwright/test";

// F0.2 — Scénarios Gherkin :
//   « inscription et vérification » : je m'inscris, je reçois un e-mail,
//   je clique le lien, mon compte est vérifié.
//   La connexion/déconnexion fonctionne de bout en bout.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

function uniqueEmail(): string {
  return `troqueur-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
}

async function signup(page: Page, email: string, pseudo: string) {
  await page.goto("/inscription");
  await page.getByLabel("Pseudo").fill(pseudo);
  await page.getByLabel("E-mail").fill(email);
  // exact : l'œil « Afficher le mot de passe » porte aussi ce libellé.
  await page.getByLabel("Mot de passe", { exact: true }).fill("un-bon-mot-de-passe");
  await page.getByLabel("Code postal").fill("44000");
  await page.getByRole("button", { name: "Créer mon compte" }).click();
  await expect(page).toHaveURL(/\/verification$/);
}

async function verificationLink(email: string): Promise<string> {
  let messageId = "";
  await expect
    .poll(
      async () => {
        const response = await fetch(
          `${MAILPIT}/api/v1/search?query=${encodeURIComponent(`to:${email}`)}`,
        );
        const body = (await response.json()) as { messages?: Array<{ ID: string }> };
        messageId = body.messages?.[0]?.ID ?? "";
        return messageId;
      },
      { timeout: 15000 },
    )
    .not.toBe("");

  const response = await fetch(`${MAILPIT}/api/v1/message/${messageId}`);
  const message = (await response.json()) as { Text: string };
  const match = message.Text.match(/https?:\/\/[^\s]+\/api\/auth\/verify-email\?token=[A-Za-z0-9_-]+/);
  if (!match) throw new Error(`Lien de vérification introuvable dans : ${message.Text}`);
  return match[0];
}

test("inscription, e-mail de vérification, lien cliqué : compte vérifié", async ({ page }) => {
  const email = uniqueEmail();
  await signup(page, email, `camille${Date.now() % 100000}`);

  await expect(
    page.getByRole("heading", { name: "Vérifie ta boîte mail" }),
  ).toBeVisible();
  await expect(page.getByText(email)).toBeVisible();

  const link = await verificationLink(email);
  await page.goto(link);

  await expect(page).toHaveURL(/\/verification\?statut=ok/);
  await expect(page.getByText("C'est tout bon !")).toBeVisible();

  // Le bandeau « vérifie ton e-mail » a disparu de l'accueil.
  await page.goto("/");
  await expect(page.getByText("Vérifie ton e-mail pour troquer")).toHaveCount(0);
});

test("compte non vérifié : le bandeau de vérification est visible", async ({ page }) => {
  const email = uniqueEmail();
  await signup(page, email, `robin${Date.now() % 100000}`);

  await page.goto("/");
  await expect(page.getByText("Vérifie ton e-mail pour troquer l'esprit tranquille")).toBeVisible();
});

test("déconnexion puis reconnexion", async ({ page }) => {
  const email = uniqueEmail();
  const pseudo = `sacha${Date.now() % 100000}`;
  await signup(page, email, pseudo);

  // Connecté : le header montre le pseudo.
  await page.goto("/");
  await expect(page.getByRole("banner").getByText(pseudo)).toBeVisible();

  // Déconnexion depuis le profil.
  await page.goto("/profil");
  await expect(page.getByRole("heading", { name: "Ton profil" })).toBeVisible();
  await page.getByRole("button", { name: "Me déconnecter" }).click();
  await expect(page.getByRole("link", { name: "M'inscrire" })).toBeVisible();

  // Reconnexion.
  await page.goto("/connexion");
  await page.getByLabel("E-mail").fill(email);
  await page.getByLabel("Mot de passe").fill("un-bon-mot-de-passe");
  await page.getByRole("button", { name: "Me connecter" }).click();
  await expect(page.getByRole("banner").getByText(pseudo)).toBeVisible();
});

test("identifiants invalides : message générique", async ({ page }) => {
  await page.goto("/connexion");
  await page.getByLabel("E-mail").fill("inconnu@exemple.fr");
  const champMdp = page.getByLabel("Mot de passe");
  await champMdp.fill("nimporte-quoi");
  // La bascule révèle puis remasque la saisie.
  await expect(champMdp).toHaveAttribute("type", "password");
  await page.getByRole("button", { name: "Afficher le mot de passe" }).click();
  await expect(champMdp).toHaveAttribute("type", "text");
  await page.getByRole("button", { name: "Masquer le mot de passe" }).click();
  await expect(champMdp).toHaveAttribute("type", "password");
  await page.getByRole("button", { name: "Me connecter" }).click();
  await expect(
    page.getByText("E-mail ou mot de passe incorrect. Vérifie et réessaie."),
  ).toBeVisible();
});
