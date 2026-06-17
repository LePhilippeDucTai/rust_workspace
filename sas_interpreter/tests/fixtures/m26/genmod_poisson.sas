/* PROC GENMOD - Poisson regression */

data poisson_data;
  input x count @@;
  datalines;
  1 2
  1 1
  2 4
  2 3
  3 6
  3 7
  4 10
  4 9
  ;
run;

proc genmod data=poisson_data;
  model count = x;
  dist=poisson;
  link=log;
  title "Poisson Regression";
run;
