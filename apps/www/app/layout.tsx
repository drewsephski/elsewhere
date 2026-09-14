import type { Metadata } from "next";
import { siteConfig } from "@elsewhere/brand";
import "./globals.css";

export const metadata: Metadata = {
  title: `${siteConfig.productName} — ${siteConfig.tagline}`,
  description: siteConfig.cloudPitch,
  openGraph: {
    title: siteConfig.productName,
    description: siteConfig.cloudPitch,
    type: "website",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <head>
        <link
          href="https://db.onlinewebfonts.com/c/0e6de1ec911a2e267ff136bbdd384a44?family=Helvetica+Neue+Light"
          rel="stylesheet"
        />
        <link
          href="https://fonts.googleapis.com/css2?family=Playfair+Display:wght@700&family=Oswald:wght@500&family=Montserrat:wght@700&family=Roboto+Slab:wght@600&family=Raleway:wght@700&display=swap"
          rel="stylesheet"
        />
      </head>
      <body className="font-helvetica-neue antialiased">{children}</body>
    </html>
  );
}
