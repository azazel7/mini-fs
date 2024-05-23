#!/bin/bash

echo "Testing $1"
FILE=$1/otter.txt

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

rm -f $FILE
if [ -f $FILE ]; then
  echo "Deletion Failed"
  exit 0
else
  echo "Deletion succeed"
fi

echo "SUCCESS"
