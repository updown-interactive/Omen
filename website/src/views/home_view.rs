// File name: home_view.rs
// App name: omen-web
// Description: Responsive landing page for the Omen framework built with Omen itself.

use omen::prelude::*;

view! {
    @Design
    pub Home {
        @Responsive layout
        @State let dark = true
        @State let active_tab = "desktop"
        @State let counter = 42
        @State let demo_toggle = true
        @State let demo_text = String::from("Rust + Omen = ❤️")
        @State let mobile_menu_open = false
        @State let copied_cmd = false

        // Theme palette
        let bg_color = if dark { "#09090B" } else { "#F8FAFC" };
        let nav_bg = if dark { "#111116" } else { "#FFFFFF" };
        let card_bg = if dark { "#141419" } else { "#FFFFFF" };
        let card_border = if dark { "#23232C" } else { "#E2E8F0" };
        let text_primary = if dark { "#FAFAFA" } else { "#0F172A" };
        let text_secondary = if dark { "#A1A1AA" } else { "#64748B" };
        let text_muted = if dark { "#71717A" } else { "#94A3B8" };
        let accent_color = "#6366F1";
        let code_bg = if dark { "#0D0D11" } else { "#18181F" };
        let badge_bg = if dark { "#1E1E2E" } else { "#EEF2FF" };
        let badge_text = if dark { "#A5B4FC" } else { "#4F46E5" };
        let stat_bg = if dark { "#16161D" } else { "#F1F5F9" };

        let page_padding = if layout.compact { 16. } else { 32. };
        let hero_spacing = if layout.compact { 20. } else { 32. };

        Layout {
            // ==========================================
            // 1. TOP NAVIGATION BAR (RESPONSIVE)
            // ==========================================
            Layout {
                Layout {
                    // Logo & Version badge
                    Layout {
                        Text("✦")
                            .fontSize(22)
                            .weight(.bold)
                            .textColor(accent_color)
                        Text("Omen")
                            .fontSize(20)
                            .weight(.bold)
                            .textColor(text_primary)
                        Text("v0.1.0")
                            .fontSize(11)
                            .weight(.semibold)
                            .textColor(badge_text)
                            .background(badge_bg)
                            .padding(4)
                            .cornerRadius(6)
                    }
                    .mode(.horizontal)
                    .spacing(10)

                    // Spacer on wide screens
                    Layout {}.fill()

                    // Desktop / Tablet Navigation Links
                    Layout {
                        Button("Features") { active_tab = "features"; }
                            .textColor(text_secondary)
                            .hoverBackground(card_border)
                            .padding(8)
                            .cornerRadius(6)
                        Button("Architecture") { active_tab = "desktop"; }
                            .textColor(text_secondary)
                            .hoverBackground(card_border)
                            .padding(8)
                            .cornerRadius(6)
                        Button("Live Demo") { counter += 1; }
                            .textColor(text_secondary)
                            .hoverBackground(card_border)
                            .padding(8)
                            .cornerRadius(6)
                        Button("Quickstart") { copied_cmd = true; }
                            .textColor(text_secondary)
                            .hoverBackground(card_border)
                            .padding(8)
                            .cornerRadius(6)
                    }
                    .mode(.horizontal)
                    .spacing(12)

                    Layout {}.fill()

                    // Action buttons (Theme Toggle & CTA)
                    Layout {
                        // Theme switch
                        Button(if dark { "☀️ Light" } else { "🌙 Dark" }) {
                            dark = !dark;
                        }
                        .fontSize(13)
                        .weight(.medium)
                        .textColor(text_primary)
                        .background(card_bg)
                        .padding(8)
                        .cornerRadius(8)
                        .hoverBackground(card_border)

                        Button(if layout.compact { if mobile_menu_open { "✕" } else { "☰" } } else { "Get Started" }) {
                            if layout.compact {
                                mobile_menu_open = !mobile_menu_open;
                            } else {
                                copied_cmd = true;
                            }
                        }
                        .fontSize(13)
                        .weight(.semibold)
                        .textColor(if layout.compact { text_primary } else { "#FFFFFF" })
                        .background(if layout.compact { card_bg } else { accent_color })
                        .padding(8)
                        .cornerRadius(8)
                    }
                    .mode(.horizontal)
                    .spacing(8)
                }
                .mode(.horizontal)
                .spacing(12)
                .padding(14)
                .cornerRadius(12)
                .background(nav_bg)
            }
            .spacing(8)

            // ==========================================
            // 2. HERO SECTION (RESPONSIVE 2-COL / 1-COL)
            // ==========================================
            Layout {
                // Announcement badge
                Layout {
                    Text("✦ RUST-FIRST DECLARATIVE UI FRAMEWORK")
                        .fontSize(12)
                        .weight(.semibold)
                        .textColor(badge_text)
                }
                .padding(8)
                .cornerRadius(20)
                .background(badge_bg)

                // Responsive Hero Body
                Layout {
                    // Left Column: Headline and Call-to-Actions
                    Layout {
                        Text("Build Native & Web Apps in Rust with SwiftUI-Like Elegance")
                            .fontSize(if layout.compact { 28 } else { 42 })
                            .weight(.bold)
                            .textColor(text_primary)
                            .lineHeight(if layout.compact { 34 } else { 50 })

                        Text("Omen combines declarative ergonomics with native hardware performance. Write once in idiomatic Rust and compile to GPUI on desktop, lightweight WASM DOM on the web, and native mobile.")
                            .fontSize(if layout.compact { 15 } else { 18 })
                            .textColor(text_secondary)
                            .lineHeight(if layout.compact { 22 } else { 28 })

                        // Hero Action Buttons
                        Layout {
                            Button("Get Started →") {
                                copied_cmd = true;
                            }
                            .fontSize(15)
                            .weight(.semibold)
                            .textColor("#FFFFFF")
                            .background(accent_color)
                            .padding(14)
                            .cornerRadius(10)
                            .hoverBackground("#4F46E5")

                            // Copy CLI command button
                            Button(if copied_cmd { "✓ Copied: omen new my-app" } else { "$ omen new my-app" }) {
                                copied_cmd = true;
                            }
                            .fontSize(14)
                            .weight(.medium)
                            .textColor(if copied_cmd { "#10B981" } else { text_primary })
                            .background(card_bg)
                            .padding(14)
                            .cornerRadius(10)
                            .hoverBackground(card_border)
                        }
                        .mode(if layout.compact { .vertical } else { .horizontal })
                        .spacing(12)

                        // Trust indicators
                        Layout {
                            Text("✓ Zero Webview Overhead")
                                .fontSize(13)
                                .textColor(text_muted)
                            Text("✓ Sub-millisecond Reactivity")
                                .fontSize(13)
                                .textColor(text_muted)
                            Text("✓ 100% Open Source")
                                .fontSize(13)
                                .textColor(text_muted)
                        }
                        .mode(if layout.compact { .vertical } else { .horizontal })
                        .spacing(16)
                    }
                    .spacing(hero_spacing)
                    .fill()

                    // Right Column: Live Interactive Demo Component
                    Layout {
                        Card {
                            // Title bar of preview card
                            Layout {
                                Text("● ● ●")
                                    .fontSize(12)
                                    .textColor(text_muted)
                                Layout {}.fill()
                                Text("Live Omen Component")
                                    .fontSize(12)
                                    .weight(.semibold)
                                    .textColor(accent_color)
                            }
                            .mode(.horizontal)

                            Divider()

                            // Interactive Counter widget
                            Layout {
                                Text("Reactive State Demo")
                                    .fontSize(13)
                                    .weight(.semibold)
                                    .textColor(text_secondary)

                                Layout {
                                    Text("Clicks: {counter}")
                                        .fontSize(24)
                                        .weight(.bold)
                                        .textColor(text_primary)

                                    Layout {}.fill()

                                    Button("+") {
                                        counter += 1;
                                    }
                                    .fontSize(16)
                                    .weight(.bold)
                                    .textColor("#FFFFFF")
                                    .background(accent_color)
                                    .padding(8)
                                    .cornerRadius(6)

                                    Button("-") {
                                        if counter > 0 { counter -= 1; }
                                    }
                                    .fontSize(16)
                                    .weight(.bold)
                                    .textColor(text_primary)
                                    .background(card_border)
                                    .padding(8)
                                    .cornerRadius(6)

                                    Button("Reset") {
                                        counter = 0;
                                    }
                                    .fontSize(12)
                                    .textColor(text_muted)
                                    .background(card_bg)
                                    .padding(6)
                                    .cornerRadius(6)
                                }
                                .mode(.horizontal)
                                .spacing(8)
                            }
                            .spacing(6)
                            .padding(12)
                            .cornerRadius(8)
                            .background(stat_bg)

                            // Interactive Toggle Switch
                            Layout {
                                Toggle("GPU Acceleration (120 FPS)", $demo_toggle)
                                Text(if demo_toggle { "Status: Hardware rendering active" } else { "Status: Low power mode" })
                                    .fontSize(12)
                                    .textColor(if demo_toggle { "#10B981" } else { text_muted })
                            }
                            .spacing(4)
                            .padding(10)
                            .cornerRadius(8)
                            .background(stat_bg)

                            // Live Input Box
                            Layout {
                                Text("Live Binding Input:")
                                    .fontSize(12)
                                    .weight(.medium)
                                    .textColor(text_secondary)
                                TextField("Enter message").value($demo_text)
                                Text("Output: {demo_text}")
                                    .fontSize(13)
                                    .weight(.semibold)
                                    .textColor(text_primary)
                            }
                            .spacing(6)
                            .padding(10)
                            .cornerRadius(8)
                            .background(stat_bg)

                            // Code snippet representation
                            Layout {
                                Text("// Omen DSL is concise & expressive:")
                                    .fontSize(11)
                                    .textColor(text_muted)
                                Text("view! {{ pub Demo {{ @State let count = {counter} }} }}")
                                    .fontSize(12)
                                    .textColor("#E9AD76")
                            }
                            .padding(8)
                            .cornerRadius(6)
                            .background(code_bg)
                        }
                        .spacing(12)
                        .padding(16)
                        .cornerRadius(16)
                        .background(card_bg)
                    }
                    .fill()
                }
                .direction(.adaptive(compact: .vertical, expanded: .horizontal))
                .spacing(24)
            }
            .spacing(20)
            .padding(page_padding)
            .cornerRadius(20)
            .background(card_bg)

            // ==========================================
            // 3. KEY METRICS & HIGHLIGHTS BAR
            // ==========================================
            AdaptiveGrid {
                Layout {
                    Text("100%").fontSize(28).weight(.bold).textColor(accent_color)
                    Text("Native Execution").fontSize(15).weight(.semibold).textColor(text_primary)
                    Text("Zero webview overhead on desktop & mobile").fontSize(13).textColor(text_secondary)
                }
                .spacing(6).padding(16).cornerRadius(12).background(card_bg)

                Layout {
                    Text("< 1ms").fontSize(28).weight(.bold).textColor("#10B981")
                    Text("State Reconciliation").fontSize(15).weight(.semibold).textColor(text_primary)
                    Text("Granular reactivity without virtual DOM diffing").fontSize(13).textColor(text_secondary)
                }
                .spacing(6).padding(16).cornerRadius(12).background(card_bg)

                Layout {
                    Text("3 in 1").fontSize(28).weight(.bold).textColor("#F59E0B")
                    Text("Unified Toolchain").fontSize(15).weight(.semibold).textColor(text_primary)
                    Text("Desktop GPUI, Web WASM DOM, and Mobile GPU").fontSize(13).textColor(text_secondary)
                }
                .spacing(6).padding(16).cornerRadius(12).background(card_bg)

                Layout {
                    Text("100%").fontSize(28).weight(.bold).textColor("#8B5CF6")
                    Text("Type Safety").fontSize(15).weight(.semibold).textColor(text_primary)
                    Text("Compile-time checked views, bindings and layouts").fontSize(13).textColor(text_secondary)
                }
                .spacing(6).padding(16).cornerRadius(12).background(card_bg)
            }
            .minItemWidth(240)
            .spacing(16)

            // ==========================================
            // 4. CORE FEATURES SECTION (ADAPTIVE GRID)
            // ==========================================
            Layout {
                Layout {
                    Text("FRAMEWORK CAPABILITIES")
                        .fontSize(12)
                        .weight(.semibold)
                        .textColor(accent_color)
                    Text("Why Rust Developers Choose Omen")
                        .fontSize(if layout.compact { 22 } else { 32 })
                        .weight(.bold)
                        .textColor(text_primary)
                    Text("Built from the ground up for modern developer happiness, exceptional rendering performance, and complete multi-platform reach.")
                        .fontSize(15)
                        .textColor(text_secondary)
                }
                .spacing(8)

                AdaptiveGrid {
                    // Feature 1
                    Card {
                        Layout {
                            Text("✦")
                                .fontSize(20)
                                .textColor(accent_color)
                            Text("Declarative Macro DSL")
                                .fontSize(17)
                                .weight(.semibold)
                                .textColor(text_primary)
                        }
                        .mode(.horizontal)
                        .spacing(8)
                        Text("Compose views effortlessly with view! and main!. Familiar SwiftUI-like syntax with Rust's strict compile-time type safety.")
                            .fontSize(14)
                            .textColor(text_secondary)
                        Text("view! · main! · Named Views")
                            .fontSize(12)
                            .textColor(badge_text)
                    }
                    .spacing(10).padding(18).cornerRadius(14).background(card_bg)

                    // Feature 2
                    Card {
                        Layout {
                            Text("⚡")
                                .fontSize(20)
                                .textColor("#10B981")
                            Text("Retained Reactive State")
                                .fontSize(17)
                                .weight(.semibold)
                                .textColor(text_primary)
                        }
                        .mode(.horizontal)
                        .spacing(8)
                        Text("@State models and two-way $bindings provide precise reactivity. Only invalidated views re-render, eliminating DOM diffing.")
                            .fontSize(14)
                            .textColor(text_secondary)
                        Text("@State · $binding · @Environment")
                            .fontSize(12)
                            .textColor(badge_text)
                    }
                    .spacing(10).padding(18).cornerRadius(14).background(card_bg)

                    // Feature 3
                    Card {
                        Layout {
                            Text("⌕")
                                .fontSize(20)
                                .textColor("#F59E0B")
                            Text("Adaptive & Responsive")
                                .fontSize(17)
                                .weight(.semibold)
                                .textColor(text_primary)
                        }
                        .mode(.horizontal)
                        .spacing(8)
                        Text("Container-aware layouts adapt to available width using AdaptiveLayout and AdaptiveGrid, plus @Responsive breakpoint hooks.")
                            .fontSize(14)
                            .textColor(text_secondary)
                        Text("AdaptiveGrid · @Responsive · Breakpoints")
                            .fontSize(12)
                            .textColor(badge_text)
                    }
                    .spacing(10).padding(18).cornerRadius(14).background(card_bg)

                    // Feature 4
                    Card {
                        Layout {
                            Text("⚙")
                                .fontSize(20)
                                .textColor("#EC4899")
                            Text("True Cross-Platform")
                                .fontSize(17)
                                .weight(.semibold)
                                .textColor(text_primary)
                        }
                        .mode(.horizontal)
                        .spacing(8)
                        Text("Zed GPUI on macOS, Linux & Windows; lean WASM DOM on the web; Vulkan/Metal on Android & iOS. No Chromium bloat.")
                            .fontSize(14)
                            .textColor(text_secondary)
                        Text("GPUI · WASM · Vulkan · Metal")
                            .fontSize(12)
                            .textColor(badge_text)
                    }
                    .spacing(10).padding(18).cornerRadius(14).background(card_bg)

                    // Feature 5
                    Card {
                        Layout {
                            Text("✓")
                                .fontSize(20)
                                .textColor("#3B82F6")
                            Text("Design Tokens & Motion")
                                .fontSize(17)
                                .weight(.semibold)
                                .textColor(text_primary)
                        }
                        .mode(.horizontal)
                        .spacing(8)
                        Text("Semantic theme tokens (.surface, .primary, .accent) that seamlessly adapt to light/dark modes, with damped spring animations.")
                            .fontSize(14)
                            .textColor(text_secondary)
                        Text(".theme() · .animation() · Springs")
                            .fontSize(12)
                            .textColor(badge_text)
                    }
                    .spacing(10).padding(18).cornerRadius(14).background(card_bg)

                    // Feature 6
                    Card {
                        Layout {
                            Text("+")
                                .fontSize(20)
                                .textColor("#8B5CF6")
                            Text("Developer Tooling & CLI")
                                .fontSize(17)
                                .weight(.semibold)
                                .textColor(text_primary)
                        }
                        .mode(.horizontal)
                        .spacing(8)
                        Text("Complete CLI toolchain with omen new and omen run, hot reload dev servers, doctor diagnostics, and the Omen Studio IDE.")
                            .fontSize(14)
                            .textColor(text_secondary)
                        Text("omen-cli · Studio Canvas · Doctor")
                            .fontSize(12)
                            .textColor(badge_text)
                    }
                    .spacing(10).padding(18).cornerRadius(14).background(card_bg)
                }
                .minItemWidth(280)
                .spacing(16)
            }
            .spacing(24)

            // ==========================================
            // 5. PLATFORM ARCHITECTURE (INTERACTIVE TABS)
            // ==========================================
            Layout {
                Layout {
                    Text("UNIFIED ARCHITECTURE")
                        .fontSize(12)
                        .weight(.semibold)
                        .textColor(accent_color)
                    Text("Target Once, Run Everywhere with Native Performance")
                        .fontSize(if layout.compact { 20 } else { 28 })
                        .weight(.bold)
                        .textColor(text_primary)
                }
                .spacing(6)

                // Architecture Cards by Platform
                AdaptiveGrid {
                    Card {
                        Layout {
                            Layout {
                                Text("Desktop: Zed's GPUI Engine")
                                    .fontSize(18)
                                    .weight(.bold)
                                    .textColor(text_primary)
                                Text("Hardware Accelerated Native Windowing")
                                    .fontSize(12)
                                    .textColor(badge_text)
                            }
                            .spacing(4)

                            Text("On macOS, Linux, and Windows, Omen uses the high-performance GPUI engine originally developed by Zed Industries. It bypasses OS widget bottlenecks to render directly to the GPU with runtime Metal and Vulkan shaders.")
                                .fontSize(13)
                                .textColor(text_secondary)
                                .lineHeight(20)

                            Layout {
                                Text("macOS (Metal)").fontSize(13).weight(.semibold).textColor(text_primary)
                                Text("Linux (Wayland / X11)").fontSize(13).weight(.semibold).textColor(text_primary)
                                Text("Windows (DirectX/Vulkan)").fontSize(13).weight(.semibold).textColor(text_primary)
                            }
                            .spacing(6)
                        }
                        .spacing(12)
                        .padding(18)
                    }
                    .background(card_bg)
                    .cornerRadius(12)

                    Card {
                        Layout {
                            Layout {
                                Text("Web: Zero-Overhead WASM + Retained DOM")
                                    .fontSize(18)
                                    .weight(.bold)
                                    .textColor(text_primary)
                                Text("Accessible, Semantic HTML with Direct WebAssembly")
                                    .fontSize(12)
                                    .textColor("#10B981")
                            }
                            .spacing(4)

                            Text("Compiles directly to wasm32-unknown-unknown. Omen's web backend reconciles retained views and drives standard HTML/CSS DOM nodes directly without React, Vue, or heavy canvas layers. Full screen-reader accessibility and browser responsiveness out of the box.")
                                .fontSize(13)
                                .textColor(text_secondary)
                                .lineHeight(20)

                            Layout {
                                Text("Zero JS Frameworks").fontSize(13).weight(.semibold).textColor(text_primary)
                                Text("Standard Accessibility (ARIA)").fontSize(13).weight(.semibold).textColor(text_primary)
                                Text("Fast Bundle Size & Rapid Startup").fontSize(13).weight(.semibold).textColor(text_primary)
                            }
                            .spacing(6)
                        }
                        .spacing(12)
                        .padding(18)
                    }
                    .background(card_bg)
                    .cornerRadius(12)

                    Card {
                        Layout {
                            Layout {
                                Text("Mobile: NativeActivity & UIKit (No Webviews)")
                                    .fontSize(18)
                                    .weight(.bold)
                                    .textColor(text_primary)
                                Text("Direct GPU Frame Delivery on Mobile")
                                    .fontSize(12)
                                    .textColor("#F59E0B")
                            }
                            .spacing(4)

                            Text("Mobile apps built with Omen run natively on Android via NativeActivity with Vulkan, and iOS via UIKit with Metal. You get native touch event processing, battery efficiency, and authentic hardware-accelerated rendering.")
                                .fontSize(13)
                                .textColor(text_secondary)
                                .lineHeight(20)

                            Layout {
                                Text("Android Vulkan (NativeActivity)").fontSize(13).weight(.semibold).textColor(text_primary)
                                Text("iOS Metal (120Hz ProMotion)").fontSize(13).weight(.semibold).textColor(text_primary)
                                Text("100% Shared State & Views").fontSize(13).weight(.semibold).textColor(text_primary)
                            }
                            .spacing(6)
                        }
                        .spacing(12)
                        .padding(18)
                    }
                    .background(card_bg)
                    .cornerRadius(12)
                }
                .minItemWidth(280)
                .spacing(16)
            }
            .spacing(18)

            // ==========================================
            // 6. CODE WALKTHROUGH: WRITE ONCE IN OMEN
            // ==========================================
            Layout {
                Layout {
                    Text("DEVELOPER EXPERIENCE")
                        .fontSize(12)
                        .weight(.semibold)
                        .textColor(accent_color)
                    Text("Expressive Syntax That Reads Like Native Rust")
                        .fontSize(if layout.compact { 20 } else { 28 })
                        .weight(.bold)
                        .textColor(text_primary)
                }
                .spacing(6)

                Layout {
                    // Code Block Card
                    Card {
                        Layout {
                            Text("src/views/profile.rs")
                                .fontSize(13)
                                .textColor(text_muted)
                            Layout {}.fill()
                            Text("Rust 2024")
                                .fontSize(11)
                                .weight(.semibold)
                                .textColor(badge_text)
                                .background(badge_bg)
                                .padding(4)
                                .cornerRadius(4)
                        }
                        .mode(.horizontal)

                        Divider()

                        // Code display
                        Layout {
                            Text("use omen::prelude::*;")
                                .fontSize(13).textColor("#818CF8")
                            Text("")
                            Text("state! {{")
                                .fontSize(13).textColor("#F472B6")
                            Text("    ProfileState {{")
                                .fontSize(13).textColor("#E0E7FF")
                            Text("        name = \"Ada Lovelace\"")
                                .fontSize(13).textColor("#34D399")
                            Text("        notifications = true")
                                .fontSize(13).textColor("#34D399")
                            Text("    }}")
                                .fontSize(13).textColor("#E0E7FF")
                            Text("}}")
                                .fontSize(13).textColor("#F472B6")
                            Text("")
                            Text("view! {{")
                                .fontSize(13).textColor("#F472B6")
                            Text("    pub ProfileView {{")
                                .fontSize(13).textColor("#E0E7FF")
                            Text("        @State state = ProfileState()")
                                .fontSize(13).textColor("#FBBF24")
                            Text("        Layout {{")
                                .fontSize(13).textColor("#E0E7FF")
                            Text("            Text(\"Hello, {{state.name}}\").font(.largeTitle)")
                                .fontSize(13).textColor("#38BDF8")
                            Text("            TextField(\"Your name\").value($state.name)")
                                .fontSize(13).textColor("#38BDF8")
                            Text("            Toggle(\"Receive alerts\", $state.notifications)")
                                .fontSize(13).textColor("#38BDF8")
                            Text("            Button(\"Save\") {{ println!(\"Saved!\"); }}")
                                .fontSize(13).textColor("#38BDF8")
                            Text("                .padding(.md).background(.accent).cornerRadius(8)")
                                .fontSize(13).textColor("#A78BFA")
                            Text("        }}.spacing(.lg).padding(.xl)")
                                .fontSize(13).textColor("#A78BFA")
                            Text("    }}")
                                .fontSize(13).textColor("#E0E7FF")
                            Text("}}")
                                .fontSize(13).textColor("#F472B6")
                        }
                        .spacing(2)
                        .padding(14)
                        .cornerRadius(10)
                        .background(code_bg)
                    }
                    .spacing(12)
                    .padding(18)
                    .cornerRadius(16)
                    .background(card_bg)
                    .fill()

                    // Side Explanation
                    Layout {
                        Card {
                            Text("1. Zero Boilerplate State")
                                .fontSize(16).weight(.bold).textColor(text_primary)
                            Text("state! generates reactive property stores with fine-grained subscriptions. No manual listeners or atom wrappers.")
                                .fontSize(13).textColor(text_secondary)
                        }
                        .spacing(6).padding(14).cornerRadius(12).background(card_bg)

                        Card {
                            Text("2. Two-Way $bindings")
                                .fontSize(16).weight(.bold).textColor(text_primary)
                            Text("Prefix any state field with $ to create a two-way binding for controls like TextField, Checkbox, or Toggle.")
                                .fontSize(13).textColor(text_secondary)
                        }
                        .spacing(6).padding(14).cornerRadius(12).background(card_bg)

                        Card {
                            Text("3. Compile-Time Checking")
                                .fontSize(16).weight(.bold).textColor(text_primary)
                            Text("Macros validate layout nesting, modifier contexts, and argument types at compile time. Catch mistakes early.")
                                .fontSize(13).textColor(text_secondary)
                        }
                        .spacing(6).padding(14).cornerRadius(12).background(card_bg)
                    }
                    .spacing(12)
                    .fill()
                }
                .direction(.adaptive(compact: .vertical, expanded: .horizontal))
                .spacing(20)
            }
            .spacing(18)

            // ==========================================
            // 7. GET STARTED IN 3 STEPS
            // ==========================================
            Layout {
                Layout {
                    Text("GET STARTED IN SECONDS")
                        .fontSize(12)
                        .weight(.semibold)
                        .textColor(accent_color)
                    Text("From Zero to Running App in Minutes")
                        .fontSize(if layout.compact { 20 } else { 28 })
                        .weight(.bold)
                        .textColor(text_primary)
                }
                .spacing(6)

                AdaptiveGrid {
                    Card {
                        Text("STEP 01").fontSize(12).weight(.bold).textColor(accent_color)
                        Text("Install Omen CLI").fontSize(16).weight(.bold).textColor(text_primary)
                        Text("Install the official project generator and build manager via Cargo.").fontSize(13).textColor(text_secondary)
                        Layout {
                            Text("cargo install omen-cli").fontSize(13).weight(.medium).textColor("#34D399")
                        }
                        .padding(10).cornerRadius(8).background(code_bg)
                    }
                    .spacing(10).padding(16).cornerRadius(14).background(card_bg)

                    Card {
                        Text("STEP 02").fontSize(12).weight(.bold).textColor(accent_color)
                        Text("Scaffold Your Project").fontSize(16).weight(.bold).textColor(text_primary)
                        Text("Create a new starter project pre-configured for desktop, web, or mobile.").fontSize(13).textColor(text_secondary)
                        Layout {
                            Text("omen new my-app && cd my-app").fontSize(13).weight(.medium).textColor("#34D399")
                        }
                        .padding(10).cornerRadius(8).background(code_bg)
                    }
                    .spacing(10).padding(16).cornerRadius(14).background(card_bg)

                    Card {
                        Text("STEP 03").fontSize(12).weight(.bold).textColor(accent_color)
                        Text("Run Everywhere").fontSize(16).weight(.bold).textColor(text_primary)
                        Text("Launch instantly on desktop or start the live-reloading browser server.").fontSize(13).textColor(text_secondary)
                        Layout {
                            Text("omen run   # or: omen run -p web").fontSize(13).weight(.medium).textColor("#34D399")
                        }
                        .padding(10).cornerRadius(8).background(code_bg)
                    }
                    .spacing(10).padding(16).cornerRadius(14).background(card_bg)
                }
                .minItemWidth(260)
                .spacing(16)
            }
            .spacing(18)

            // ==========================================
            // 8. CALL TO ACTION BANNER
            // ==========================================
            Layout {
                Card {
                    Layout {
                        Text("✦")
                            .fontSize(32)
                            .textColor("#FFFFFF")

                        Text("Start Building the Future of Rust UI Today")
                            .fontSize(if layout.compact { 22 } else { 32 })
                            .weight(.bold)
                            .textColor("#FFFFFF")

                        Text("Join developers crafting lightning-fast, beautifully responsive native and web applications.")
                            .fontSize(15)
                            .textColor("#E0E7FF")

                        Layout {
                            Button("Get Started Now →") {
                                copied_cmd = true;
                            }
                            .fontSize(15)
                            .weight(.semibold)
                            .textColor(accent_color)
                            .background("#FFFFFF")
                            .padding(14)
                            .cornerRadius(10)

                            Button("Explore GitHub Repo") {
                                copied_cmd = true;
                            }
                            .fontSize(14)
                            .weight(.medium)
                            .textColor("#FFFFFF")
                            .background("#4338CA")
                            .padding(14)
                            .cornerRadius(10)
                        }
                        .mode(if layout.compact { .vertical } else { .horizontal })
                        .spacing(12)
                    }
                    .spacing(16)
                    .padding(if layout.compact { 20 } else { 36 })
                }
                .cornerRadius(20)
                .background("#4F46E5")
            }

            // ==========================================
            // 9. FOOTER
            // ==========================================
            Layout {
                Divider()

                Layout {
                    Layout {
                        Layout {
                            Text("✦ Omen").fontSize(16).weight(.bold).textColor(text_primary)
                            Text("Declarative UI Framework for Rust").fontSize(12).textColor(text_muted)
                        }
                        .spacing(2)

                        Text("Built for speed, ergonomics, and seamless multi-platform delivery.")
                            .fontSize(13)
                            .textColor(text_secondary)
                    }
                    .spacing(6)
                    .fill()

                    Layout {
                        Text("Open Source (MIT or Apache-2.0)")
                            .fontSize(13)
                            .textColor(text_secondary)
                        Text("© 2026 UpDown Interactive. All rights reserved.")
                            .fontSize(12)
                            .textColor(text_muted)
                    }
                    .spacing(4)
                }
                .direction(.adaptive(compact: .vertical, expanded: .horizontal))
                .spacing(16)
            }
            .spacing(16)
            .padding(16)
        }
        .spacing(36)
        .padding(page_padding)
        .frame(.infinity, .infinity)
        .scrollable()
        .background(bg_color)
        .theme(if dark { .dark } else { .light })
        .safeArea(.all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omen::web::*;
    use omen::Viewport;

    fn find<'a>(node: &'a DomNode, text: &str) -> Option<&'a DomNode> {
        if node.text == text
            || node
                .attributes
                .get("aria-label")
                .is_some_and(|label| label == text)
        {
            Some(node)
        } else {
            node.children.iter().find_map(|child| find(child, text))
        }
    }

    #[test]
    fn landing_page_renders_in_compact_mobile_viewport() {
        let mut engine = WebEngine::new(view! { Home() });
        let phone_viewport = Viewport {
            width: 375.,
            height: 667.,
            ..Default::default()
        };
        let scene = engine.render(phone_viewport, false, false).expect("render compact");
        assert!(find(&scene.root, "Omen").is_some());
        assert!(find(&scene.root, "v0.1.0").is_some());
        // Verify mobile menu button is present in compact mode
        assert!(find(&scene.root, "☰").is_some());
    }

    #[test]
    fn landing_page_renders_in_medium_tablet_viewport() {
        let mut engine = WebEngine::new(view! { Home() });
        let tablet_viewport = Viewport {
            width: 768.,
            height: 1024.,
            ..Default::default()
        };
        let scene = engine.render(tablet_viewport, false, false).expect("render medium");
        assert!(find(&scene.root, "Features").is_some());
        assert!(find(&scene.root, "Architecture").is_some());
    }

    #[test]
    fn landing_page_renders_in_expanded_desktop_viewport() {
        let mut engine = WebEngine::new(view! { Home() });
        let desktop_viewport = Viewport {
            width: 1440.,
            height: 900.,
            ..Default::default()
        };
        let scene = engine.render(desktop_viewport, false, false).expect("render desktop");
        assert!(find(&scene.root, "Get Started").is_some());
        assert!(find(&scene.root, "Live Omen Component").is_some());
        assert!(find(&scene.root, "Clicks: 42").is_some());
    }

    #[test]
    fn landing_page_interactive_counter_and_theme_toggle() {
        let mut engine = WebEngine::new(view! { Home() });
        let viewport = Viewport {
            width: 1200.,
            height: 800.,
            ..Default::default()
        };
        let scene = engine.render(viewport, false, false).unwrap();

        // Check increment button
        let inc_btn = find(&scene.root, "+").expect("increment button").id.clone();
        assert!(engine.dispatch(BrowserEvent::Activate { id: inc_btn }));

        let scene = engine.render(viewport, false, false).unwrap();
        assert!(find(&scene.root, "Clicks: 43").is_some());

        // Check theme toggle button
        let theme_btn = find(&scene.root, "☀️ Light").expect("theme button").id.clone();
        assert!(engine.dispatch(BrowserEvent::Activate { id: theme_btn }));

        let scene = engine.render(viewport, false, false).unwrap();
        assert!(find(&scene.root, "🌙 Dark").is_some());
    }
}

