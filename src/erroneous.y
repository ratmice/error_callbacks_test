%token a b
%%
A -> (): B a | C {};
B -> (): a | a a {};
C -> (): %empty | D | C b {};
D -> (): %empty | b {};
