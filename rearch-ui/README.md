# rearch-ui

Not sure how you found your way here, but this is just me fooling around with some prototype UI code
for a (possible) future UI framework built around ReArch.

See `lib.rs` for more:
```rust
fn sample_view(_: ViewHandle, _: ()) -> TerminatedView {
    view()
        // If you decide to use the builtin proc macro:
        .padding(16.0) // sugar for .child(padding, 16.0) without proc macro

        // Views to align things:
        .center() // sugar for .child(center, ())

        // We will also support scoping state:
        .inject(scoped_state, 1234) // injects whatever scoped_state returns into all descendants
        // A similar macro will exist for scoping state: .scoped_state(1234)

        // And of course you can have views with multiple children:
        .row(Default::default())
        .children(vec![
            view().child(text, "Hello World!".to_owned()),
            view()
                .inject(text_style, TextStyle)
                .child(text, "Hello World Again!".to_owned()),
            view()
                .child(padding, 16.0)
                .child(column, (0, 0.0))
                .children(vec![
                    view().child(text, "A list item:".to_owned()),
                    view()
                        .inject(scoped_index_key::<usize>, 0)
                        .child(list_item, ()),
                ]),
        ])
}
```
Kinda cool, right?
An entirely functional, declarative way to develop your UI applications.
And macro free if you so choose!

`rearch-ui` would only supply a UI frontend to build apps with,
leaving the heavy lifting of actually rendering/similar
up to a swappable backend implementation,
allowing easy cross-platform support.

If I ever do finish this prototype, I may then make a backend implementation via Flutter.
While it's not optimal by any stretch of the imagination
(compared to hand rolling a backend per platform),
it'll at least be a starting point and will support all platforms out of the box
until platform-specific implementations can be made.
