# cairo-svgpath

Draw on cairo surfaces using SVG path syntax, with compile-time parsing and conversion.

```rust
use cairo_svgpath::svgpath;

fn before(ctx: &cairo::Context) {
	ctx.move_to(1, 2);
	ctx.line_to(3, 4);
	ctx.curve_to(5, 6, 7, 8, 9, 10);
}

fn after(ctx: &cairo::Context) {
	svgpath!(ctx, "M1 2L3 4C5 6 7 8 9 10");
}
```
