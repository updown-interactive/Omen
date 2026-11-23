//! Non-interactive device chrome drawn around Studio's measured preview viewport.
use gpui::{Bounds, BoxShadow, Div, point, prelude::*, px, rgb, rgba, size};
use omen_core::PreviewShell;

pub(super) fn render(element: Div, shell: PreviewShell, dark: bool, scale: f32) -> Div {
    use PreviewShell::*;
    let [side, top, bottom] = shell.insets();
    let unit = move |v| px(v * scale);
    let radius = match shell {
        IPhone | MobileBrowser => 31.,
        AndroidPhone => 24.,
        IPad | TabletBrowser => 19.,
        AndroidTablet => 14.,
        Mac | DesktopBrowser => 10.,
        Windows | Linux => 7.,
        Custom => 3.,
    };
    let hardware = matches!(
        shell,
        IPhone | AndroidPhone | IPad | AndroidTablet | MobileBrowser | TabletBrowser
    );
    let (edge, face, shade) = if hardware {
        (0x78818b, 0x353b43, 0x13171c)
    } else if dark {
        (0x575d67, 0x3a3e46, 0x252830)
    } else {
        (0xb3b9c3, 0xf1f3f6, 0xd5dae2)
    };
    // Insets reserve all hardware and window controls outside the document's coordinate space.
    element
        .debug_selector(|| "studio-device-shell".into())
        .relative()
        .pl(unit(side))
        .pr(unit(side))
        .pt(unit(top))
        .pb(unit(bottom))
        .rounded(unit(radius))
        .bg(gpui::linear_gradient(
            155.,
            gpui::linear_color_stop(rgb(face), 0.),
            gpui::linear_color_stop(rgb(shade), 1.),
        ))
        .shadow(vec![
            BoxShadow::new(unit(0.), unit(7.), rgba(0x060a1440).into()).blur_radius(unit(15.)),
            BoxShadow::new(unit(0.), unit(1.), rgba(0x080b1238).into()).blur_radius(unit(3.)),
        ])
        .child(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    let width = f32::from(bounds.size.width) / scale;
                    let height = f32::from(bounds.size.height) / scale;
                    window.paint_quad(gpui::quad(
                        bounds,
                        unit(radius),
                        gpui::transparent_black(),
                        unit(1.),
                        rgb(edge),
                        gpui::BorderStyle::Solid,
                    ));

                    let mut shape = |x, y, w, h, r, color| {
                        let origin = bounds.origin + point(unit(x), unit(y));
                        window.paint_quad(
                            gpui::fill(Bounds::new(origin, size(unit(w), unit(h))), rgb(color))
                                .corner_radii(unit(r)),
                        );
                    };
                    let mid = width / 2.;
                    let ink = if dark || hardware { 0xb3bcc8 } else { 0x646e7c };
                    match shell {
                        IPhone => {
                            shape(mid - 27., 10., 54., 11., 5.5, 0x090b0e);
                            shape(mid + 13., 13., 5., 5., 2.5, 0x253747);
                        }
                        AndroidPhone | AndroidTablet | IPad => {
                            let y = top / 2.;
                            shape(mid - 4., y - 4., 8., 8., 4., 0x080a0d);
                            shape(mid - 1.5, y - 1.5, 3., 3., 1.5, 0x304658);
                        }
                        Mac => {
                            for (x, color) in [(12., 0xf37670), (26., 0xeaba59), (40., 0x67bf82)] {
                                shape(x, top / 2. - 4., 8., 8., 4., color);
                            }
                        }
                        Windows => {
                            shape(width - 62., 15., 8., 1., 0., ink);
                            shape(width - 42., 11., 8., 7., 1., ink);
                            shape(width - 41., 12., 6., 5., 0., face);
                            for i in 0..7 {
                                let i = i as f32;
                                shape(width - 20. + i, 11. + i, 1.4, 1.4, 0., ink);
                                shape(width - 14. - i, 11. + i, 1.4, 1.4, 0., ink);
                            }
                        }
                        Linux => {
                            shape(mid - 15., 13., 30., 3., 1.5, ink);
                            shape(width - 23., 9., 11., 11., 5.5, ink);
                        }
                        MobileBrowser | TabletBrowser | DesktopBrowser | Custom => {}
                    }
                    if matches!(shell, MobileBrowser | TabletBrowser | DesktopBrowser) {
                        let bar = (width * 0.58).min(480.);
                        let bar_color = if dark || hardware { 0x4a505a } else { 0xc4cbd4 };
                        shape(mid - bar / 2., (top - 18.) / 2., bar, 18., 9., bar_color);
                        for i in 0..3 {
                            shape(width - 19. + i as f32 * 3., top / 2. - 1., 2., 2., 1., ink);
                        }
                    }
                    if matches!(shell, IPhone | AndroidPhone | MobileBrowser) {
                        let bar = (width * 0.26).min(100.);
                        shape(mid - bar / 2., height - bottom / 2., bar, 3., 1.5, 0xb7bec6);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
}
