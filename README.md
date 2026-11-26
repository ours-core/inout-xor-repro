# inout-xor-repro

`StreamCipher::apply_keystream` from RustCrypto spends most of its time XORing the keystream
into the data in `inout`'s `InOut::xor_in2out`, not in AES. At `opt-level = "s"` the XOR is one
byte at a time and the cipher runs at a third of the speed of the same keystream XORed by hand;
at `opt-level = 3` the gap is still about 40%.

Versions (see `Cargo.lock`): `inout` 0.2.6, `cipher` 0.5.2, `ctr` 0.10.1, `aes` 0.9.4.

## Run

```
cargo run --locked --profile release      # fat LTO, codegen-units = 1, opt-level = "s"
cargo run --locked --profile release-o3   # the same at opt-level = 3
cargo run --locked --profile release-hot  # "s", with only the cipher crates and the caller at 3
```

The program encrypts 1 GiB in 16 KiB pieces with `ctr::Ctr128BE<aes::Aes256>` two ways and
prints the best of five runs of each:

- `apply_keystream` — the crate's API;
- by words — `write_keystream` into a 2 KiB stack buffer, then XOR 16 bytes at a time
  (`u128::from_ne_bytes`).

It first asserts that both produce the same bytes.

## Results

AMD EPYC 9554 (Zen 4, VAES), one core, `rustc 1.100.0` (pinned in `rust-toolchain.toml`):

| profile | `apply_keystream` | by words | ratio |
|---|---|---|---|
| `opt-level = "s"`, fat LTO | 1.19 GB/s | 3.96 GB/s | 3.3× |
| `opt-level = 3`, fat LTO | 5.79 GB/s | 8.29 GB/s | 1.4× |

The `release-hot` profile raises only `aes`, `ctr`, `cipher`, `hybrid-array` and the caller to
`opt-level = 3` (not `inout`), and brings `apply_keystream` back to 6.01 GB/s (by words: 8.28 GB/s). We saw the byte-wise loop in a larger fat-LTO binary configured that
way too, but have not reduced that case to a minimal example yet.

## Where the time goes at `opt-level = "s"`

The two hottest instructions in `perf`, about 53% of all samples together, are the inner loop of
`xor_in2out` inlined into the CTR backend:

```
mov    (%rcx,%rdi,1),%r8b
xor    (%rax,%rdi,1),%r8b
mov    %r8b,(%r15,%rdi,1)
inc    %rdi
cmp    $0x10,%rdi
jne    ...
```

AES itself (`vaesenc` on zmm) takes about 21%. The XOR by words compiles to `xorps` at the same
`opt-level = "s"`.

The function copies the input into a temporary `Array`, XORs it byte by byte and writes the
temporary back:

```rust
let input = ptr::read(self.in_ptr);
let mut temp = Array::<u8, N>::default();
for i in 0..N::USIZE {
    temp[i] = input[i] ^ data[i];
}
ptr::write(self.out_ptr, temp);
```

At `opt-level = "s"` LLVM does not vectorize this loop; at `opt-level = 3` it does, but the round
trip through `temp` still costs about 40% against XORing words in place.

## License

MIT-0: use it for anything, no attribution needed.
