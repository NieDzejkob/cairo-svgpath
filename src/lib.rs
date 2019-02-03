#![feature(proc_macro_hygiene)]
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn svgpath(input: TokenStream) -> TokenStream {
    let (ident, path) = get_params(input).expect("syntax error");
    let mut path: Path = path.parse().expect("parsing the path failed");
    simplify(&mut path)
        .map(|segment| match segment {
            PathSegment::MoveTo { x, y, .. } => quote! { #ident.move_to(#x, #y); },
            PathSegment::LineTo { x, y, .. } => quote! { #ident.line_to(#x, #y); },
            PathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
                ..
            } => quote! { #ident.curve_to(#x1, #y1, #x2, #y2, #x, #y); },
            other => {
                dbg!(other);
                unimplemented!();
            }
        })
        .map(Into::<proc_macro::TokenStream>::into)
        .collect()
}

fn get_params(input: TokenStream) -> Option<(proc_macro2::Ident, String)> {
    let input: proc_macro2::TokenStream = input.into();
    let mut input = input.into_iter();
    let ident = input.next()?;
    match input.next()? {
        proc_macro2::TokenTree::Punct(punct) => {
            if punct.as_char() != ',' {
                return None;
            }
        }
        _ => return None,
    }

    let ident = match ident {
        proc_macro2::TokenTree::Ident(ident) => ident,
        _ => return None,
    };

    let path = input.next()?;
    // Handle trailing input
    if let Some(_) = input.next() {
        return None;
    };

    let path = match path {
        proc_macro2::TokenTree::Literal(literal) => syn::Lit::new(literal),
        _ => return None,
    };

    let path = match path {
        syn::Lit::Str(string) => string.value(),
        _ => return None,
    };

    Some((ident, path))
}

use svgtypes::{Path, PathSegment};

// Adapted from resvg/usvg because the apes don't feel like marking their conversion routines as
// pub. As a bonus, this uses half the amount of curves for arcs, yielding better runtime
// performance. Did I mention I'm still amazed one can simply run a tesselator as a macro for this?
fn simplify<'a>(path: &'a mut Path) -> impl Iterator<Item = PathSegment> + 'a {
    path.conv_to_absolute();
    // Previous MoveTo coordinates. Used for ClosePath.
    let mut pmx = 0.0;
    let mut pmy = 0.0;

    // Previous coordinates.
    let mut px = 0.0;
    let mut py = 0.0;

    // Previous SmoothQuadratic coordinates.
    let mut ptx = 0.0;
    let mut pty = 0.0;

    let mut previous = None;

    path.iter().flat_map(move |&seg| {
        let segs = match seg {
            PathSegment::MoveTo { .. } => vec![seg],
            PathSegment::LineTo { .. } => vec![seg],
            PathSegment::HorizontalLineTo { x, .. } => vec![PathSegment::LineTo {
                x,
                y: py,
                abs: true,
            }],
            PathSegment::VerticalLineTo { y, .. } => vec![PathSegment::LineTo {
                x: px,
                y,
                abs: true,
            }],
            PathSegment::CurveTo { .. } => vec![seg],
            PathSegment::SmoothCurveTo { x2, y2, x, y, .. } => {
                let (x1, y1) = if let Some(prev_seg) = previous {
                    match prev_seg {
                        PathSegment::CurveTo { x2, y2, x, y, .. }
                        | PathSegment::SmoothCurveTo { x2, y2, x, y, .. } => {
                            (x * 2.0 - x2, y * 2.0 - y2)
                        }
                        _ => (px, py),
                    }
                } else {
                    (px, py)
                };

                vec![PathSegment::CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                    abs: true,
                }]
            }
            PathSegment::Quadratic { x1, y1, x, y, .. } => vec![handle_quadratic(px, py, x1, y1, x, y)],
            PathSegment::SmoothQuadratic { x, y, .. } => {
                let (x1, y1) = if let Some(prev_seg) = previous {
                    match prev_seg {
                        PathSegment::Quadratic { x1, y1, x, y, .. } => (x * 2.0 - x1, y * 2.0 - y1),
                        PathSegment::SmoothQuadratic { x, y, .. } => (x * 2.0 - ptx, y * 2.0 - pty),
                        _ => (px, py),
                    }
                } else {
                    (px, py)
                };

                ptx = x1;
                pty = y1;

                vec![handle_quadratic(px, py, x1, y1, x, y)]
            }
            PathSegment::EllipticalArc { rx, ry, x_axis_rotation, large_arc, sweep, x, y, .. } => {
                let arc = lyon_geom::SvgArc {
                    from: [px, py].into(),
                    to: [x, y].into(),
                    radii: [rx, ry].into(),
                    x_rotation: euclid::Angle::degrees(x_axis_rotation),
                    flags: lyon_geom::ArcFlags { large_arc, sweep },
                };

                let mut curves = vec![];
                arc.for_each_cubic_bezier(&mut |curve| {
                    curves.push(PathSegment::CurveTo {
                        x1: curve.ctrl1.x,
                        y1: curve.ctrl1.y,
                        x2: curve.ctrl2.x,
                        y2: curve.ctrl2.y,
                        x: curve.to.x,
                        y: curve.to.y,
                        abs: true,
                    });
                });
                curves
            }
            PathSegment::ClosePath { .. } => {
                if let Some(PathSegment::ClosePath { .. }) = previous {
                    // skip consecutive closes
                    vec![]
                } else {
                    vec![seg]
                }
            }
        };

        if let Some(&seg) = segs.last() {
            match seg {
                PathSegment::MoveTo { x, y, .. } => {
                    px = x;
                    py = y;
                    pmx = x;
                    pmy = y;
                }
                PathSegment::LineTo { x, y, .. }
                | PathSegment::CurveTo { x, y, .. } => {
                    px = x;
                    py = y;
                }
                PathSegment::ClosePath { .. } => {
                    px = pmx;
                    py = pmy;
                }
                _ => unreachable!(),
            }
        }

        previous = Some(seg);
        segs.into_iter()
    })
}

fn handle_quadratic(px: f64, py: f64, x1: f64, y1: f64, x: f64, y: f64) -> PathSegment {
    PathSegment::CurveTo {
        x,
        y,
        x1: 2.0 / 3.0 * x1 + 1.0 / 3.0 * px,
        y1: 2.0 / 3.0 * y1 + 1.0 / 3.0 * py,
        x2: 2.0 / 3.0 * x1 + 1.0 / 3.0 * x,
        y2: 2.0 / 3.0 * y1 + 1.0 / 3.0 * y,
        abs: true,
    }
}
