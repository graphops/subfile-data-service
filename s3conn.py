import boto3
from botocore.client import Config

# Initialize a session using DigitalOcean Spaces.
session = boto3.session.Session()
client = session.client('s3',
                        region_name=process.env.BUCKET,
                        endpoint_url=process.env.S3_URL,
                        aws_access_key_id=process.env.AWS_ACCESS_KEY_ID,
                        aws_secret_access_key=process.env.AWS_SECRET_ACCESS_KEY)

# List all buckets on your account.
response = client.list_buckets()
spaces = [space['Name'] for space in response['Buckets']]
print("Spaces List: %s" % spaces)


# List all objects in the bucket.
response = client.list_objects(Bucket = 'contain-texture-dragon')
print("Objects List: %s" % response)

# Put an object 
response = client.put_object(Bucket = 'contain-texture-dragon', Key = "blahblahblah")
print("Put result: %s" % response)

# # Delete an object 
# response = client.delete_object(Bucket = 'contain-texture-dragon', Key = 'DO002U62QEJCZLT7UK6D')

# List all objects in the bucket.
response = client.list_objects(Bucket = 'contain-texture-dragon')
print("Objects List: %s" % response)

# Read an object
response = client.get_object(Bucket = 'contain-texture-dragon', Key = "blahblahblah")
print("Read result: %s" % response)
