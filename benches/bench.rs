use criterion::{Criterion, criterion_group, criterion_main};
use re_finite_automata::*;
use std::hint::black_box;

pub fn bench_nfa_shortest_no_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(1..=255);
    c.bench_function("NFA shortest no match", |b| {
        b.iter(|| black_box(nfa.run_shortest(&mut input.iter().copied())))
    });
}

pub fn bench_nfa_shortest_char_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(0..=0);
    c.bench_function("NFA shortest character match", |b| {
        b.iter(|| black_box(nfa.run_shortest(&mut input.iter().copied())))
    });
}

pub fn bench_nfa_shortest_4_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(0..=255);
    let nfa = nfa.clone() + nfa.clone() + nfa.clone() + nfa;
    c.bench_function("NFA shortest 4 char match", |b| {
        b.iter(|| black_box(nfa.run_shortest(&mut input.iter().copied())))
    });
}

pub fn bench_nfa_shortest_either_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(0..=1);
    let nfa = nfa.clone() | nfa;
    c.bench_function("NFA shortest either match", |b| {
        b.iter(|| black_box(nfa.run_shortest(&mut input.iter().copied())))
    });
}

pub fn bench_nfa_no_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(1..=255);
    c.bench_function("NFA no match", |b| b.iter(|| black_box(nfa.run(&*input))));
}

pub fn bench_nfa_char_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(0..=0);
    c.bench_function("NFA character match", |b| {
        b.iter(|| black_box(nfa.run(&*input)))
    });
}

pub fn bench_nfa_4_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(0..=255);
    let nfa = nfa.clone() + nfa.clone() + nfa.clone() + nfa;
    c.bench_function("NFA 4 char match", |b| {
        b.iter(|| black_box(nfa.run(&*input)))
    });
}

pub fn bench_nfa_either_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let nfa = Nfa::from_range(0..=1);
    let nfa = nfa.clone() | nfa;
    c.bench_function("NFA either match", |b| {
        b.iter(|| black_box(nfa.run(&*input)))
    });
}

pub fn bench_dfa_no_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let dfa = Dfa::from_range(1..=255);
    c.bench_function("DFA no match", |b| {
        b.iter(|| black_box(dfa.run(&mut input.iter().copied())))
    });
}

pub fn bench_dfa_char_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let dfa = Dfa::from_range(0..=0);
    c.bench_function("DFA character match", |b| {
        b.iter(|| black_box(dfa.run(&mut input.iter().copied())))
    });
}

pub fn bench_dfa_4_match(c: &mut Criterion) {
    let input: Vec<u8> = black_box((0..1000).map(|_| 0).collect());
    let dfa = Dfa::from_range(0..=255);
    let dfa = dfa.clone() + dfa.clone() + dfa.clone() + dfa;
    c.bench_function("DFA 4 char match", |b| {
        b.iter(|| black_box(dfa.run(&mut input.iter().copied())))
    });
}

criterion_group!(
    benches,
    bench_nfa_shortest_no_match,
    bench_nfa_shortest_char_match,
    bench_nfa_shortest_4_match,
    bench_nfa_shortest_either_match,
    bench_nfa_no_match,
    bench_nfa_char_match,
    bench_nfa_4_match,
    bench_nfa_either_match,
    bench_dfa_no_match,
    bench_dfa_char_match,
    bench_dfa_4_match,
);
criterion_main!(benches);
