import { siteConfig } from "@elsewhere/brand";

interface AuthEmailPayload {
  to: string;
  subject: string;
  text: string;
  html?: string;
}

/** Sends transactional auth emails via Resend when configured; logs in development otherwise. */
export async function sendAuthEmail(payload: AuthEmailPayload): Promise<void> {
  const apiKey = process.env.RESEND_API_KEY?.trim();
  const from =
    process.env.AUTH_EMAIL_FROM?.trim() ??
    `${siteConfig.productName} <${siteConfig.contactEmail}>`;

  if (!apiKey) {
    console.info("[auth email]", {
      to: payload.to,
      subject: payload.subject,
      text: payload.text,
    });
    return;
  }

  const response = await fetch("https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      from,
      to: [payload.to],
      subject: payload.subject,
      text: payload.text,
      html: payload.html ?? payload.text,
    }),
  });

  if (!response.ok) {
    const body = await response.text();
    console.error("Failed to send auth email", response.status, body);
    throw new Error("Failed to send email");
  }
}
