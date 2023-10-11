use std::marker::PhantomData;

use criterion::*;

fn bench(c: &mut Criterion) {
    // Slash *> Literal("users") *> Slash *> StringVar <* Slash <* Literal("posts") <* Slash <*> IntVar
    let parser = Slash
        .zip_right(Literal(String::from("users")))
        .zip_right(Slash)
        .zip_right(StringVar)
        .zip_left(Slash)
        .zip_left(Literal(String::from("posts")))
        .zip_left(Slash)
        .zip(IntVar);

    let users = Literal(String::from("users"));
    let posts = Literal(String::from("posts"));

    println!("{:?}", parser.parse("/users/jdegoes/posts/123"));
    println!("{:?}", hardcoded_parse("/users/jdegoes/posts/123"));
    println!(
        "{:?}",
        hardcoded2_parse(&users, &posts, "/users/jdegoes/posts/123"),
    );

    let mut group = c.benchmark_group("bench");
    group.throughput(Throughput::Elements(1));
    group.bench_function("classic", |b| {
        b.iter(|| parser.parse(black_box("/users/jdegoes/posts/123")))
    });
    group.bench_function("hardcoded", |b| {
        b.iter(|| hardcoded_parse(black_box("/users/jdegoes/posts/123")))
    });
    group.bench_function("hardcoded2", |b| {
        b.iter(|| hardcoded2_parse(&users, &posts, black_box("/users/jdegoes/posts/123")))
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);

trait RouteParser<'path, A>: Sized {
    fn parse(&self, path: &'path str) -> Option<(A, &'path str)>;

    fn combine_with<B, C, RouteParserB, FnABC>(
        self,
        that: RouteParserB,
        f: FnABC,
    ) -> Combine<'path, A, B, C, Self, RouteParserB, FnABC>
    where
        RouteParserB: RouteParser<'path, B>,
        FnABC: Fn(A, B) -> C,
    {
        Combine {
            left: self,
            right: that,
            f,
            _phantom: PhantomData,
        }
    }

    fn map<B, FnAB>(self, f: FnAB) -> Map<'path, A, B, Self, FnAB>
    where
        FnAB: Fn(A) -> B,
    {
        Map {
            parser: self,
            f,
            _phantom: PhantomData,
        }
    }

    fn zip<B, RouteParserB>(
        self,
        that: RouteParserB,
    ) -> Combine<'path, A, B, (A, B), Self, RouteParserB, fn(A, B) -> (A, B)>
    where
        RouteParserB: RouteParser<'path, B>,
    {
        self.combine_with(that, |a, b| (a, b))
    }

    fn zip_left<B, RoutePaserB>(
        self,
        that: RoutePaserB,
    ) -> Combine<'path, A, B, A, Self, RoutePaserB, fn(A, B) -> A>
    where
        RoutePaserB: RouteParser<'path, B>,
    {
        self.combine_with(that, |a, _| a)
    }

    fn zip_right<B, RoutePaserB>(
        self,
        that: RoutePaserB,
    ) -> Combine<'path, A, B, B, Self, RoutePaserB, fn(A, B) -> B>
    where
        RoutePaserB: RouteParser<'path, B>,
    {
        self.combine_with(that, |_, b| b)
    }
}

struct Literal(String);
impl<'path> RouteParser<'path, ()> for Literal {
    fn parse(&self, path: &'path str) -> Option<((), &'path str)> {
        if path.starts_with(&self.0) {
            Some(((), &path[self.0.len()..]))
        } else {
            None
        }
    }
}

struct Slash;
impl<'path> RouteParser<'path, ()> for Slash {
    fn parse(&self, path: &'path str) -> Option<((), &'path str)> {
        if path.starts_with('/') {
            Some(((), &path[1..]))
        } else {
            None
        }
    }
}

struct StringVar;
impl<'path> RouteParser<'path, &'path str> for StringVar {
    fn parse(&self, path: &'path str) -> Option<(&'path str, &'path str)> {
        let idx = path.find('/');
        match idx {
            None => Some((path, "")),
            Some(idx) => Some((&path[..idx], &path[idx..])),
        }
    }
}

struct IntVar;
impl<'path> RouteParser<'path, i32> for IntVar {
    fn parse(&self, path: &'path str) -> Option<(i32, &'path str)> {
        let idx = path.find('/');
        match idx {
            None => path.parse().ok().map(|int| (int, "")),
            Some(idx) => {
                let seg = &path[..idx];

                seg.parse().ok().map(|int| (int, &path[idx..]))
            }
        }
    }
}

struct Map<'path, A, B, RouteParserA, FnAB>
where
    RouteParserA: RouteParser<'path, A>,
    FnAB: Fn(A) -> B,
{
    parser: RouteParserA,
    f: FnAB,
    _phantom: PhantomData<fn(&'path A) -> B>,
}

impl<'path, A, B, RouteParserA, FnAB> RouteParser<'path, B> for Map<'path, A, B, RouteParserA, FnAB>
where
    RouteParserA: RouteParser<'path, A>,
    FnAB: Fn(A) -> B,
{
    fn parse(&self, path: &'path str) -> Option<(B, &'path str)> {
        self.parser.parse(path).map(|(a, rest)| ((self.f)(a), rest))
    }
}

struct Combine<'path, A, B, C, RouteParserA, RouteParserB, FnABC>
where
    RouteParserA: RouteParser<'path, A>,
    RouteParserB: RouteParser<'path, B>,
    FnABC: Fn(A, B) -> C,
{
    left: RouteParserA,
    right: RouteParserB,
    f: FnABC,
    _phantom: PhantomData<fn(&'path A, &'path B) -> C>,
}

impl<'path, A, B, C, RouteParserA, RouteParserB, FnABC> RouteParser<'path, C>
    for Combine<'path, A, B, C, RouteParserA, RouteParserB, FnABC>
where
    RouteParserA: RouteParser<'path, A>,
    RouteParserB: RouteParser<'path, B>,
    FnABC: Fn(A, B) -> C,
{
    fn parse(&self, path: &'path str) -> Option<(C, &'path str)> {
        self.left.parse(path).and_then(|(a, path)| {
            self.right
                .parse(path)
                .map(|(b, path)| ((self.f)(a, b), path))
        })
    }
}

fn hardcoded_parse(path: &str) -> Option<((&str, i32), &str)> {
    let mut remaining_path = path;

    let literal1 = "/";
    if remaining_path.starts_with(literal1) {
        remaining_path = &remaining_path[literal1.len()..];
    } else {
        return None;
    }

    let literal2 = "users";
    if remaining_path.starts_with(literal2) {
        remaining_path = &remaining_path[literal2.len()..];
    } else {
        return None;
    }

    let literal3 = "/";
    if remaining_path.starts_with(literal3) {
        remaining_path = &remaining_path[literal3.len()..];
    } else {
        return None;
    }

    let string_var = {
        let idx = remaining_path.find('/');
        match idx {
            None => {
                let string_var = remaining_path;
                remaining_path = "";
                string_var
            }
            Some(idx) => {
                let string_var = &remaining_path[..idx];
                remaining_path = &remaining_path[idx..];
                string_var
            }
        }
    };

    let literal4 = "/";
    if remaining_path.starts_with(literal4) {
        remaining_path = &remaining_path[literal4.len()..];
    } else {
        return None;
    }

    let literal5 = "posts";
    if remaining_path.starts_with(literal5) {
        remaining_path = &remaining_path[literal5.len()..];
    } else {
        return None;
    }

    let literal6 = "/";
    if remaining_path.starts_with(literal6) {
        remaining_path = &remaining_path[literal6.len()..];
    } else {
        return None;
    }

    let int_var = {
        let idx = remaining_path.find('/');
        match idx {
            None => {
                if let Some(int) = remaining_path.parse().ok() {
                    remaining_path = "";
                    int
                } else {
                    return None;
                }
            }
            Some(idx) => {
                let seg = &remaining_path[..idx];

                if let Some(int) = seg.parse().ok() {
                    remaining_path = &remaining_path[idx..];
                    int
                } else {
                    return None;
                }
            }
        }
    };

    Some(((string_var, int_var), remaining_path))
}

fn hardcoded2_parse<'path>(
    users: &Literal,
    posts: &Literal,
    path: &'path str,
) -> Option<((&'path str, i32), &'path str)> {
    let remaining_path = path;

    let ((), remaining_path) = Slash.parse(remaining_path)?;
    let ((), remaining_path) = users.parse(remaining_path)?;
    let ((), remaining_path) = Slash.parse(remaining_path)?;
    let (string_var, remaining_path) = StringVar.parse(remaining_path)?;
    let ((), remaining_path) = Slash.parse(remaining_path)?;
    let ((), remaining_path) = posts.parse(remaining_path)?;
    let ((), remaining_path) = Slash.parse(remaining_path)?;
    let (int_var, remaining_path) = IntVar.parse(remaining_path)?;

    Some(((string_var, int_var), remaining_path))
}
