#!/bin/bash

echo "Testing $1"
MOUNT_POINT=$1
FILE=$MOUNT_POINT/otter.txt

touch $FILE
if [ -f $FILE ]; then
  echo "Creation succeed"
else
  echo "Creation Failed"
  exit 0
fi

DATA="Otter will conquer the world!"
echo $DATA > $FILE
reading=`cat $FILE`
if [ "$DATA" = "$reading" ]; then
  echo "Small read/write succeed"
else
  echo "Small read/write failed"
  exit 0
fi

DATA="Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vestibulum leo neque, dapibus nec facilisis sollicitudin, blandit nec orci. Aenean convallis mi ante, sit amet tempor lacus sollicitudin eget. Aliquam lacinia nisl et massa vulputate, id facilisis felis fringilla. Nulla rhoncus mollis sodales. Praesent vitae faucibus elit, convallis malesuada est. Nam quam quam, ultricies in posuere vel, feugiat at nulla. Vivamus id tristique eros. Mauris maximus justo in nisi bibendum eleifend vestibulum id ligula. Aliquam in velit dui. Nam euismod vitae lorem id tempus. Nulla interdum accumsan mauris non mollis. Vestibulum sollicitudin faucibus rhoncus. Nulla quis mollis ligula. Nam imperdiet pulvinar quam, id gravida arcu rhoncus vel. Donec tempor quam magna, eget eleifend mi molestie ac. Duis vitae malesuada urna. Nullam commodo tortor nec fermentum rutrum. Donec nisl urna, ultrices id malesuada eget, consectetur eget nulla. Etiam ut posuere neque, vitae mollis metus. Fusce bibendum tincidunt."
echo $DATA > $FILE
reading=`cat $FILE`
if [ "$DATA" = "$reading" ]; then
  echo "Big read/write succeed"
else
  echo "Big read/write failed"
  exit 0
fi

DATA="Pony are the best bet!"
echo $DATA > $FILE
reading=`cat $FILE`
if [ "$DATA" = "$reading" ]; then
  echo "Small after read/write succeed"
else
  echo "Small after read/write failed"
  exit 0
fi

rm -f $FILE
if [ -f $FILE ]; then
  echo "Deletion Failed"
  exit 0
else
  echo "Deletion succeed"
fi

d1="Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vestibulum leo neque, dapibus nec facilisis sollicitudin, blandit nec orci. Aenean convallis mi ante, sit amet tempor lacus sollicitudin eget. A"
d2="liquam lacinia nisl et massa vulputate, id facilisis felis fringilla. Nulla rhoncus mollis sodales. Praesent vitae faucibus elit, convallis malesuada est. Nam quam quam, ultricies in posuere vel, feugiat a"
d3="t nulla. Vivamus id tristique eros. Mauris maximus justo in nisi bibendum eleifend vestibulum id ligula. Aliquam in velit dui. Nam euismod vitae lorem id tempus. Nulla interdum accumsan mauris non mollis."
d4=" Vestibulum sollicitudin faucibus rhoncus. Nulla quis mollis ligula. Nam imperdiet pulvinar quam, id gravida arcu rhoncus vel. Donec tempor quam magna, eget eleifend mi molestie ac. Duis vitae malesuada"
d5=" urna. Nullam commodo tortor nec fermentum rutrum. Donec nisl urna, ultrices id malesuada eget, consectetur eget nulla. Etiam ut posuere neque, vitae mollis metus. Fusce bibendum tincidunt."
DIR1=$MOUNT_POINT/dir1
DIR2=$MOUNT_POINT/dir2
FILE1=$DIR1/canard.txt
FILE2=$DIR2/loutre.txt
FILE3=$DIR1/loutre.txt
mkdir -p $DIR1
mkdir -p $DIR2

echo "$d1" > $FILE1
echo "$d1" > $FILE2
echo "$d1" > $FILE3
echo "$d2" >> $FILE1
echo "$d2" >> $FILE2
echo "$d2" >> $FILE3
echo "$d3" >> $FILE1
echo "$d3" >> $FILE2
echo "$d3" >> $FILE3
echo "$d4" >> $FILE1
echo "$d4" >> $FILE2
echo "$d4" >> $FILE3
echo "$d5" >> $FILE1
echo "$d5" >> $FILE2
echo "$d5" >> $FILE3

reading1=`cat $FILE1`
reading2=`cat $FILE2`
reading3=`cat $FILE3`
# The > and >> operators add new lines
DATA=$d1$'\n'$d2$'\n'$d3$'\n'$d4$'\n'$d5

if [ "$DATA" = "$reading1" ] && [ "$DATA" = "$reading2" ] && [ "$DATA" = "$reading3" ]; then
  echo "Intertwinned read/write succeed"
else
  echo "Intertwinned read/write failed"
  exit 0
fi

rm -rf $DIR1
rm -rf $DIR2

echo "SUCCESS"
