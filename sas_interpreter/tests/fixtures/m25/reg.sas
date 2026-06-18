/* M25.1 — PROC REG : OLS weight = height on the sashelp.class clone.
   Oracle (documented SAS): Intercept -143.02692 (SE 32.27459, t -4.43,
   p 0.0004), height 3.89903 (SE 0.51609, t 7.55, p <.0001); R-Square 0.7705,
   Adj R-Sq 0.7570, Root MSE 11.22625, Dependent Mean 100.02632; ANOVA Model
   SS 7193.24912 (DF1), Error SS 2142.48772 (DF17), Total 9335.73684 (DF18),
   F 57.08. OUTPUT OUT= adds predicted (pred) and residual (resid). */

libname d 'data';

title "PROC REG: weight = height";
proc reg data=d.class;
    model weight = height;
    output out=fit p=pred r=resid;
run;

title "REG OUTPUT: predicted & residual";
proc print data=fit;
    var name height weight pred resid;
run;
title;
