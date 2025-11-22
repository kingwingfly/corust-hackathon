After implementing trait for tuples, we'll got a dirty documentation on docs.rs:
```text
impl<'de, T> Deserialize<'de> for (T0,)
impl<'de, T> Deserialize<'de> for (T0, T1)
impl<'de, T> Deserialize<'de> for (T0, T1, T2)
impl<'de, T> Deserialize<'de> for (T0, T1, T2, T3)
..
```

But if we go to `serde`'s documentation, we'd get pretty
```text
impl<'de, T> Deserialize<'de> for (T₁, T₂, …, Tₙ)
```

It is because serde uses `fake_variadic`.
