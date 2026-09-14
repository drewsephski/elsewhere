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
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link
          href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Playfair+Display:wght@700&family=Oswald:wght@500&family=Montserrat:wght@700&family=Roboto+Slab:wght@600&display=swap"
          rel="stylesheet"
        />
      </head>
      <body className="font-sans antialiased">{children}</body>
    </html>
  );
}
