/* M25.2 — PROC ANOVA : balanced one-way (growth by fert), 3 levels x 3 reps.
   Oracle (hand-computed): group means A=2, B=5, C=8, grand 5; Model SS=54
   (DF2), Error SS=6 (DF6), Corrected Total 60 (DF8); MS 27/1; F=27.00;
   R-Square 0.9. MEANS fert prints level means 2/5/8. */

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

title "PROC ANOVA: one-way growth by fert (balanced)";
proc anova data=plants;
    class fert;
    model growth = fert;
    means fert;
run;
title;
