import { siteConfig } from "@elsewhere/brand";

interface AuthEmailPayload {
  to: string;
  subject: string;
  text: string;
  html?: string;
}

function isProductionAuthEmail(): boolean {
  return process.env.NODE_ENV === "production";
}

/** Sends transactional auth emails via Resend when configured; logs safely in local development only. */
export async function sendAuthEmail(payload: AuthEmailPayload): Promise<void> {
  const apiKey = process.env.RESEND_API_KEY?.trim();
  const from =
    process.env.AUTH_EMAIL_FROM?.trim() ??
    `${siteConfig.productName} <${siteConfig.contactEmail}>`;

  if (!apiKey) {
    if (isProductionAuthEmail()) {
      throw new Error("Transactional email is not configured for this environment");
    }
    console.info("[auth email]", {
      to: payload.to,
      subject: payload.subject,
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
