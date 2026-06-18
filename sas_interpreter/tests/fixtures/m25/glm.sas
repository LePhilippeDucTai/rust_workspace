/* M25.3 — PROC GLM : (1) continuous model == PROC REG ; (2) one-way CLASS
   with SOLUTION + LSMEANS.
   Oracle (1) weight=height on d.class: slope 3.89903, intercept -143.02692,
   Type I SS = Type III SS = 7193.24912, F 57.08, R-Square 0.7705.
   Oracle (2) growth=fert (balanced, A ref): Type I = Type III "fert" SS=54,
   F=27.00, R-Square 0.9 ; reference-cell SOLUTION Intercept=2 (level A mean),
   fert B=3 (5-2), fert C=6 (8-2) ; LSMEANS fert = 2/5/8 (= raw means). */

libname d 'data';

title "PROC GLM: weight = height (continuous, == REG)";
proc glm data=d.class;
    model weight = height / solution;
run;

data plants;
    input fert $ growth;
    datalines;
A 1
A 2
A 3
B 4
B 5
B 6
C 7
C 8
C 9
;
run;

title "PROC GLM: growth = fert (CLASS, SOLUTION, LSMEANS)";
proc glm data=plants;
    class fert;
    model growth = fert / solution;
    lsmeans fert;
run;
title;
