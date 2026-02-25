# Design Specification: Absolute Zero (kiro-rs Admin UI)

This document provides exact values, classes, and structural guidelines to implement the "Absolute Zero" visual direction using React 18, Tailwind CSS 3, Radix UI (shadcn), and `lottie-react`.

## 1. Design Tokens

Update your global `globals.css` (or `index.css`) with these exact HSL values.

```css
@layer base {
  :root {
    --background: 0 0% 0%;
    --foreground: 0 0% 100%;
    --card: 0 0% 2%;
    --card-foreground: 0 0% 98%;
    --popover: 0 0% 3%;
    --popover-foreground: 0 0% 98%;
    --primary: 0 0% 100%;
    --primary-foreground: 0 0% 0%;
    --secondary: 240 5% 10%;
    --secondary-foreground: 0 0% 98%;
    --muted: 240 5% 12%;
    --muted-foreground: 240 5% 65%;
    --accent: 240 5% 15%;
    --accent-foreground: 0 0% 98%;
    --destructive: 348 100% 50%;
    --destructive-foreground: 0 0% 100%;
    --warning: 48 100% 50%;
    --success: 142 100% 50%;
    --border: 0 0% 12%;
    --input: 0 0% 12%;
    --ring: 0 0% 100%;
    --radius: 0.5rem;
  }
}
```

### Font Stack
- Sans (Primary): `Inter, SF Pro Display, -apple-system, sans-serif`
- Mono (Data): `JetBrains Mono, Fira Code, monospace`

## 2. Global Styles & Effects

### Background Treatment
Fixed div covering viewport: `fixed inset-0 z-[-1] bg-black` with SVG noise overlay at 4% opacity.

```css
background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 200 200' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)' opacity='0.04'/%3E%3C/svg%3E");
```

### Spotlight Cursor Hover Effect
Attach `onMouseMove` to dashboard grid container, pass `--mouse-x`, `--mouse-y` CSS variables.

```css
.bento-card::before {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: radial-gradient(
    800px circle at var(--mouse-x) var(--mouse-y),
    rgba(255, 255, 255, 0.06),
    transparent 40%
  );
  opacity: 0;
  transition: opacity 0.5s;
  pointer-events: none;
}
.bento-card:hover::before { opacity: 1; }
```

### Global Animations
Default transition: `transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]`

## 3. Per-Component Specification

### UI Primitives (shadcn/ui overrides)

1. **Button**: Primary `bg-white text-black hover:bg-neutral-200 border-none font-medium`. Secondary `bg-transparent border border-white/20 text-white hover:bg-white/10 hover:border-white/40 backdrop-blur-sm`.
2. **Card**: `bg-[#050505] border border-white/10 shadow-none overflow-hidden relative group rounded-lg bento-card`.
3. **Input**: `bg-transparent border-white/15 focus-visible:ring-0 focus-visible:border-white/50 text-white placeholder:text-neutral-600 font-mono text-sm h-11`.
4. **Badge**: `border border-white/20 bg-transparent text-neutral-300 font-mono text-xs uppercase tracking-widest px-2 py-0.5 rounded-full`.
5. **Switch**: Unchecked track `bg-neutral-800` thumb `bg-neutral-400`. Checked track `bg-white` thumb `bg-black`.
6. **Checkbox**: `border-white/30 data-[state=checked]:bg-white data-[state=checked]:text-black`.
7. **Dialog**: Overlay `bg-black/80 backdrop-blur-md`. Content `bg-[#080808] border border-white/15 shadow-[0_0_80px_-20px_rgba(255,255,255,0.1)] rounded-xl`.
8. **Progress**: Track `bg-white/10 h-1.5 rounded-full`. Indicator `bg-gradient-to-r from-neutral-500 to-white`.
9. **Sonner**: `bg-[#0A0A0A] border border-white/15 text-white font-mono shadow-2xl rounded-lg`.

### Complex Components

1. **credential-card.tsx**: Flex column. Top: auth method + Lottie status dot. Middle: huge monospace ID (`font-mono text-xl tracking-tight text-white`). Bottom: balance progress + failure count. Spotlight cursor effect on hover.
2. **add-credential-dialog.tsx & kam-import-dialog.tsx**: Clean single-column forms. Success Lottie checkmark on submit.
3. **balance-dialog.tsx**: Massive balance number (`text-5xl font-mono font-light text-white`), progress bar underneath.
4. **batch-import/verify-dialog.tsx**: Textarea `font-mono text-xs bg-[#030303] border-white/10 p-4`. Loading Lottie overlay during verification.
5. **kiro-oauth-dialog.tsx**: Center large OAuth Lottie animation. "Authenticating with Kiro..." in `text-neutral-400 text-sm animate-pulse`.

## 4. Login Page

- Background: Pure black + large (600x600px) looping ambient Lottie at `opacity-20`
- Card: `max-w-md w-full bg-black/50 backdrop-blur-2xl border-t border-white/20` sides/bottom `border-white/5`
- Title: "KIRO-RS" in `font-mono text-2xl tracking-[0.3em] font-light`
- Inputs: Bottom border only `border-b border-white/20 border-x-0 border-t-0 rounded-none px-0`
- Entrance: Staggered fade-up animation `opacity:0 translateY(20px) -> fadeUp 0.8s ease`

## 5. Dashboard Layout

- Grid: `grid grid-cols-1 md:grid-cols-12 gap-4 p-6 max-w-[1600px] mx-auto`
- Header (span 12): Left "KIRO-RS // MISSION CONTROL" `font-mono text-xs text-neutral-500`. Right: action buttons.
- Hero Stats (3 cards, each col-span-4): Total Balance, Active Credentials, API Failure Rate. Huge numbers, thin fonts.
- Credential Grid (col-span-12): `grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4`
- API Key Section (col-span-12): Flat table. Headers `text-xs text-neutral-600 font-sans uppercase`, rows `text-sm font-mono border-b border-white/5`.

## 6. Lottie Animation Inventory

| ID | Purpose | Trigger | Loop | Search Terms |
|----|---------|---------|------|-------------|
| anim_ambient | Login/Dashboard background | Page load | Infinite | "abstract slow fluid", "mesh gradient black and white" |
| anim_status_ok | Active credential indicator | status == active | Infinite | "breathing dot white", "radar pulse minimal" |
| anim_status_err | Error credential indicator | status == error | Loop | "glitch x", "static noise red" |
| anim_status_warn | Rate limited indicator | status == rate_limited | Infinite | "dashed spinning ring", "loading circle minimal" |
| anim_success | Dialog success feedback | API 200 OK | Play Once | "minimalist checkmark draw" |
| anim_loading | Loading state | isLoading == true | Infinite | "geometric loading", "liquid ring spinner" |
| anim_oauth | OAuth dialog animation | Dialog open | Infinite | "secure fingerprint", "interlocking rings white" |
