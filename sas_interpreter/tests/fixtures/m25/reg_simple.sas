/* PROC REG - simple linear regression */

data regdata;
  input x y @@;
  datalines;
  1 2.1
  2 4.0
  3 6.2
  4 7.9
  5 10.1
  ;
run;

proc reg data=regdata alpha=0.05;
  model y = x;
  title "Simple Linear Regression";
run;
