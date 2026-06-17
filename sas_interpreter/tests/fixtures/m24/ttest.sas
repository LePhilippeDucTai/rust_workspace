/* M24.2 — PROC TTEST : one-sample, two-sample (CLASS), paired.
   Oracles (sashelp.class clone + inline trial):
   - one-sample height vs H0=62 : mean=62.336842, std=5.127075, n=19,
     stderr=1.176233, t=0.28637, df=18, p~0.778, 95% CL [59.866, 64.808].
   - two-sample height by sex (F first by sas_cmp) : meanF=60.5889 (n9),
     meanM=63.9100 (n10), diff=-3.3211 ; pooled t~-1.45 df17 p~0.16 ;
     Satterthwaite t~-1.45 df~16.8 ; folding F for equal variances.
   - paired pre*post on d=pre-post=[-2,-3,-2,-4] : mean=-2.75, std=0.95743,
     stderr=0.478714, t=-5.7446, df=3, p~0.0105. */

libname d 'data';

title "One-sample t-test: height vs H0=62";
proc ttest data=d.class h0=62;
    var height;
run;

title "Two-sample t-test: height by sex";
proc ttest data=d.class;
    class sex;
    var height;
run;

data trial;
    input pre post;
    datalines;
10 12
12 15
14 16
16 20
;
run;

title "Paired t-test: pre vs post";
proc ttest data=trial;
    paired pre*post;
run;
title;
