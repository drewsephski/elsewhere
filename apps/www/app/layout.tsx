import type { Metadata } from "next";
import { siteConfig } from "@elsewhere/brand";
import "./globals.css";
import { Geist } from "next/font/google";
import { cn } from "@/lib/utils";

const geist = Geist({subsets:['latin'],variable:'--font-sans'});

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
    <html lang="en" className={cn("font-sans", geist.variable)} suppressHydrationWarning>
      <body className="font-sans antialiased">{children}</body>
    </html>
  );
}
